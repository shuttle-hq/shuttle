use std::future::Future;
use std::io;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use axum::extract::{ConnectInfo, Path, State};
use axum::headers::{HeaderMapExt, Host};
use axum::response::{IntoResponse, Response};
use axum::routing::any;
use axum_server::accept::DefaultAcceptor;
use axum_server::tls_rustls::RustlsAcceptor;
use fqdn::{fqdn, FQDN};
use futures::prelude::*;
use http::header::SERVER;
use http::{HeaderValue, StatusCode};
use hyper::body::{Body, HttpBody};
use hyper::client::connect::dns::GaiResolver;
use hyper::client::HttpConnector;
use hyper::{Client, Request};
use hyper_reverse_proxy::ReverseProxy;
use once_cell::sync::Lazy;
use opentelemetry::global;
use opentelemetry_http::HeaderInjector;
use shuttle_backends::cache::{CacheManagement, CacheManager};
use shuttle_backends::headers::XShuttleProject;
use shuttle_backends::project_name::ProjectName;
use shuttle_common::constants::DEPLOYER_SERVICE_HTTP_PORT;
use shuttle_common::models::error::{
    ApiError, InvalidProjectName, ProjectNotReady, ProjectUnavailable,
};
use thiserror::Error;
use tokio::net::TcpSocket;
use tokio::sync::mpsc::Sender;
use tower_sanitize_path::SanitizePath;
use tracing::{debug, debug_span, error, field, trace, Instrument};
use tracing_opentelemetry::OpenTelemetrySpanExt;

use crate::acme::AcmeClient;
use crate::service::{self, GatewayService};
use crate::task::BoxedTask;

static PROXY_CLIENT: Lazy<ReverseProxy<HttpConnector<GaiResolver>>> =
    Lazy::new(|| ReverseProxy::new(Client::new()));
static SERVER_HEADER: Lazy<HeaderValue> = Lazy::new(|| "shuttle.rs".parse().unwrap());

#[derive(Debug, Error)]
enum Error {
    #[error("The 'Host' header is invalid")]
    BadHost,

    #[error(transparent)]
    InvalidProjectName(#[from] InvalidProjectName),

    #[error(transparent)]
    ProjectNotReady(#[from] ProjectNotReady),

    #[error(transparent)]
    ProjectUnavailable(#[from] ProjectUnavailable),

    #[error(transparent)]
    Service(#[from] service::Error),
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        // Make the exposed error safe first
        let message = match self {
            Error::BadHost => self.to_string(),
            Error::InvalidProjectName(e) => e.to_string(),
            Error::ProjectNotReady(e) => e.to_string(),
            Error::ProjectUnavailable(e) => e.to_string(),
            Error::Service(e) => {
                let error: ApiError = e.into();

                error.message
            }
        };

        // Use a custom 600 status code to distinguish between proxy errors and project errors
        (StatusCode::from_u16(600).unwrap(), message).into_response()
    }
}

pub struct ProxyState {
    gateway: Arc<GatewayService>,
    task_sender: Sender<BoxedTask>,
    public: FQDN,
    project_cache: CacheManager<IpAddr>,
    domain_cache: CacheManager<ProjectName>,
}

async fn proxy(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(state): State<Arc<ProxyState>>,
    mut req: Request<Body>,
) -> Result<Response, Error> {
    let span = debug_span!("proxy", http.method = %req.method(), http.host = field::Empty, http.uri = %req.uri(), http.status_code = field::Empty, shuttle.project.name = field::Empty);
    trace!(?req, "serving proxy request");

    let fqdn = req
        .headers()
        .typed_get::<Host>()
        .map(|host| fqdn!(host.hostname()))
        .ok_or_else(|| Error::BadHost)?;

    span.record("http.host", fqdn.to_string());

    let project_name =
        if fqdn.is_subdomain_of(&state.public) && fqdn.depth() - state.public.depth() == 1 {
            fqdn.labels()
                .next()
                .unwrap()
                .to_owned()
                .parse()
                .map_err(|_| InvalidProjectName)?
        } else if let Some(project) = { state.domain_cache.get(fqdn.to_string().as_str()) } {
            project
        } else {
            let project_name = state
                .gateway
                .project_details_for_custom_domain(&fqdn)
                .await?
                .project_name;
            state.domain_cache.insert(
                fqdn.to_string().as_str(),
                project_name.clone(),
                std::time::Duration::from_millis(5000),
            );
            project_name
        };

    // Record current project for tracing purposes
    span.record("shuttle.project.name", &project_name.to_string());

    req.headers_mut()
        .typed_insert(XShuttleProject(project_name.to_string()));

    // cache project ip lookups to not overload the db during rapid requests
    let target_ip = if let Some(ip) = { state.project_cache.get(project_name.as_str()) } {
        ip
    } else {
        let (proj, was_stopped) = state
            .gateway
            .find_or_start_project(&project_name, state.task_sender.clone())
            .await?;
        let ip = proj.state.target_ip().ok_or(ProjectNotReady)?;
        state.project_cache.insert(
            project_name.as_str(),
            ip,
            std::time::Duration::from_millis(1000),
        );

        if was_stopped {
            // wait until service has started and opens its port, give up after 10s
            span.in_scope(|| debug!("project waking up, checking service port"));

            let addr = SocketAddr::new(ip, DEPLOYER_SERVICE_HTTP_PORT);
            let _ = tokio::time::timeout(Duration::from_secs(10), async move {
                let mut ms = 5;
                loop {
                    if let Ok(socket) = TcpSocket::new_v4() {
                        if socket.connect(addr).await.is_ok() {
                            debug!("service port detected open");
                            break;
                        }
                    }
                    trace!("waiting for service port to open");
                    // exponential backoff
                    tokio::time::sleep(Duration::from_millis(ms)).await;
                    ms *= 2;
                }
            })
            .instrument(span.clone())
            .await;
        }

        ip
    };
    let target_url = format!("http://{}:{}", target_ip, DEPLOYER_SERVICE_HTTP_PORT);

    let cx = span.context();
    global::get_text_map_propagator(|propagator| {
        propagator.inject_context(&cx, &mut HeaderInjector(req.headers_mut()))
    });

    let mut res = PROXY_CLIENT
        .call(addr.ip(), &target_url, req)
        .await
        .map_err(|err| {
            error!(error = ?err, "gateway proxy client error");
            ProjectUnavailable
        })?;

    res.headers_mut().insert(SERVER, SERVER_HEADER.clone());
    let (parts, body) = res.into_parts();
    let body = <Body as HttpBody>::map_err(body, axum::Error::new).boxed_unsync();

    span.record("http.status_code", parts.status.as_u16());

    Ok(Response::from_parts(parts, body))
}

#[derive(Clone)]
pub struct Bouncer {
    gateway: Arc<GatewayService>,
    public: FQDN,
}

async fn bounce(State(state): State<Arc<Bouncer>>, req: Request<Body>) -> Result<Response, Error> {
    let mut resp = Response::builder();

    let host = req.headers().typed_get::<Host>().unwrap();
    let hostname = host.hostname();
    let fqdn = fqdn!(hostname);

    let path = req.uri();

    if fqdn.is_subdomain_of(&state.public)
        || state
            .gateway
            .project_details_for_custom_domain(&fqdn)
            .await
            .is_ok()
    {
        resp = resp
            .status(301)
            .header("Location", format!("https://{hostname}{path}"));
    } else {
        resp = resp.status(404);
    }

    let body = <Body as HttpBody>::map_err(Body::empty(), axum::Error::new).boxed_unsync();

    Ok(resp.body(body).unwrap())
}

#[derive(Default)]
pub struct UserServiceBuilder {
    service: Option<Arc<GatewayService>>,
    task_sender: Option<Sender<BoxedTask>>,
    acme: Option<AcmeClient>,
    tls_acceptor: Option<RustlsAcceptor<DefaultAcceptor>>,
    bouncer_binds_to: Option<SocketAddr>,
    user_binds_to: Option<SocketAddr>,
    public: Option<FQDN>,
}

impl UserServiceBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_public(mut self, public: FQDN) -> Self {
        self.public = Some(public);
        self
    }

    pub fn with_service(mut self, service: Arc<GatewayService>) -> Self {
        self.service = Some(service);
        self
    }

    pub fn with_task_sender(mut self, task_sender: Sender<BoxedTask>) -> Self {
        self.task_sender = Some(task_sender);
        self
    }

    pub fn with_bouncer(mut self, bound_to: SocketAddr) -> Self {
        self.bouncer_binds_to = Some(bound_to);
        self
    }

    pub fn with_user_proxy_binding_to(mut self, bound_to: SocketAddr) -> Self {
        self.user_binds_to = Some(bound_to);
        self
    }

    pub fn with_acme(mut self, acme: AcmeClient) -> Self {
        self.acme = Some(acme);
        self
    }

    pub fn with_tls(mut self, acceptor: RustlsAcceptor<DefaultAcceptor>) -> Self {
        self.tls_acceptor = Some(acceptor);
        self
    }

    pub fn serve(self) -> impl Future<Output = Result<(), io::Error>> {
        let service = self.service.expect("a GatewayService is required");
        let task_sender = self.task_sender.expect("a task sender is required");
        let public = self.public.expect("a public FQDN is required");
        let user_binds_to = self
            .user_binds_to
            .expect("a socket address to bind to is required");

        let san = SanitizePath::sanitize_paths(
            axum::Router::new()
                .fallback(proxy) // catch all routes
                .with_state(Arc::new(ProxyState {
                    gateway: service.clone(),
                    task_sender,
                    public: public.clone(),
                    project_cache: CacheManager::new(1024),
                    domain_cache: CacheManager::new(256),
                })),
        );
        let user_proxy = axum::ServiceExt::into_make_service_with_connect_info::<SocketAddr>(san);

        let bouncer = self.bouncer_binds_to.as_ref().map(|_| {
            axum::Router::new()
                .fallback(bounce) // catch all routes
                .with_state(Arc::new(Bouncer {
                    gateway: service.clone(),
                    public: public.clone(),
                }))
        });

        let mut futs = Vec::new();
        if let Some(tls_acceptor) = self.tls_acceptor {
            // TLS is enabled
            let bouncer = bouncer.expect("TLS cannot be enabled without a bouncer");
            let bouncer_binds_to = self.bouncer_binds_to.unwrap();

            let acme = self
                .acme
                .expect("TLS cannot be enabled without an ACME client");

            let bouncer = axum::Router::new()
                .route(
                    "/.well-known/acme-challenge/*rest",
                    any(
                        |Path(token): Path<String>, State(client): State<AcmeClient>| async move {
                            trace!(token, "responding to certificate challenge");
                            match client.get_http01_challenge_authorization(&token).await {
                                Some(key) => Ok(key),
                                None => Err(StatusCode::NOT_FOUND),
                            }
                        },
                    ),
                )
                .with_state(acme)
                .merge(bouncer);

            let bouncer = axum_server::Server::bind(bouncer_binds_to)
                .serve(bouncer.into_make_service())
                .map(|handle| ("bouncer (with challenge responder)", handle))
                .boxed();

            futs.push(bouncer);

            let user_with_tls = axum_server::Server::bind(user_binds_to)
                .acceptor(tls_acceptor)
                .serve(user_proxy)
                .map(|handle| ("user proxy (with TLS)", handle))
                .boxed();
            futs.push(user_with_tls);
        } else {
            if let Some(bouncer) = bouncer {
                // bouncer is enabled
                let bouncer_binds_to = self.bouncer_binds_to.unwrap();
                let bouncer = axum_server::Server::bind(bouncer_binds_to)
                    .serve(bouncer.into_make_service())
                    .map(|handle| ("bouncer (without challenge responder)", handle))
                    .boxed();
                futs.push(bouncer);
            }

            let user_without_tls = axum_server::Server::bind(user_binds_to)
                .serve(user_proxy)
                .map(|handle| ("user proxy (no TLS)", handle))
                .boxed();
            futs.push(user_without_tls);
        }

        future::select_all(futs).map(|((name, resolved), _, _)| {
            error!(service = %name, "exited early");
            resolved
        })
    }
}
