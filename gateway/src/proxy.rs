use std::convert::Infallible;
use std::future::Future;
use std::io;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use axum::headers::{HeaderMapExt, Host};
use axum::response::{IntoResponse, Response};
use axum_server::accept::DefaultAcceptor;
use axum_server::tls_rustls::RustlsAcceptor;
use fqdn::{fqdn, FQDN};
use futures::future::{ready, Ready};
use futures::prelude::*;
use hyper::body::{Body, HttpBody};
use hyper::client::connect::dns::GaiResolver;
use hyper::client::HttpConnector;
use hyper::server::conn::AddrStream;
use hyper::{Client, Request};
use hyper_reverse_proxy::ReverseProxy;
use once_cell::sync::Lazy;
use opentelemetry::global;
use opentelemetry_http::HeaderInjector;
use tower::{Service, ServiceBuilder};
use tracing::{debug, debug_span, error, field, trace};
use tracing_opentelemetry::OpenTelemetrySpanExt;

use crate::acme::{AcmeClient, ChallengeResponderLayer};
use crate::service::GatewayService;
use crate::{Error, ErrorKind, ProjectName};

static PROXY_CLIENT: Lazy<ReverseProxy<HttpConnector<GaiResolver>>> =
    Lazy::new(|| ReverseProxy::new(Client::new()));

pub trait AsResponderTo<R> {
    fn as_responder_to(&self, req: R) -> Self;

    fn into_make_service(self) -> ResponderMakeService<Self>
    where
        Self: Sized,
    {
        ResponderMakeService { inner: self }
    }
}

pub struct ResponderMakeService<S> {
    inner: S,
}

impl<'r, S> Service<&'r AddrStream> for ResponderMakeService<S>
where
    S: AsResponderTo<&'r AddrStream>,
{
    type Response = S;
    type Error = Infallible;
    type Future = Ready<Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: &'r AddrStream) -> Self::Future {
        ready(Ok(self.inner.as_responder_to(req)))
    }
}

#[derive(Clone)]
pub struct UserProxy {
    gateway: Arc<GatewayService>,
    remote_addr: SocketAddr,
    public: FQDN,
}

impl<'r> AsResponderTo<&'r AddrStream> for UserProxy {
    fn as_responder_to(&self, addr_stream: &'r AddrStream) -> Self {
        let mut responder = self.clone();
        responder.remote_addr = addr_stream.remote_addr();
        responder
    }
}

impl UserProxy {
    async fn proxy(self, mut req: Request<Body>) -> Result<Response, Error> {
        let span = debug_span!("proxy", http.method = %req.method(), http.host = ?req.headers().get("Host"), http.uri = %req.uri(), http.status_code = field::Empty, project = field::Empty);
        trace!(?req, "serving proxy request");

        let project_str = req
            .headers()
            .typed_get::<Host>()
            .map(|host| fqdn!(host.hostname()))
            .and_then(|fqdn| {
                debug!(host = %fqdn, public = %self.public, "comparing host key");
                if fqdn.is_subdomain_of(&self.public) && fqdn.depth() - self.public.depth() == 1 {
                    Some(fqdn.labels().next().unwrap().to_owned())
                } else {
                    None
                }
            })
            .ok_or_else(|| Error::from_kind(ErrorKind::ProjectNotFound))?;

        let project_name: ProjectName = project_str
            .parse()
            .map_err(|_| Error::from_kind(ErrorKind::InvalidProjectName))?;

        let project = self.gateway.find_project(&project_name).await?;

        // Record current project for tracing purposes
        span.record("project", &project_name.to_string());

        let target_ip = project
            .target_ip()?
            .ok_or_else(|| Error::from_kind(ErrorKind::ProjectNotReady))?;

        let target_url = format!("http://{}:{}", target_ip, 8000);

        let cx = span.context();

        global::get_text_map_propagator(|propagator| {
            propagator.inject_context(&cx, &mut HeaderInjector(req.headers_mut()))
        });

        let proxy = PROXY_CLIENT
            .call(self.remote_addr.ip(), &target_url, req)
            .await
            .map_err(|_| Error::from_kind(ErrorKind::ProjectUnavailable))?;

        let (parts, body) = proxy.into_parts();
        let body = <Body as HttpBody>::map_err(body, axum::Error::new).boxed_unsync();

        span.record("http.status_code", parts.status.as_u16());

        Ok(Response::from_parts(parts, body))
    }
}

impl Service<Request<Body>> for UserProxy {
    type Response = Response;
    type Error = Error;
    type Future =
        Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send + 'static>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        self.clone()
            .proxy(req)
            .or_else(|err: Error| future::ready(Ok(err.into_response())))
            .boxed()
    }
}

#[derive(Clone)]
pub struct Bouncer {
    gateway: Arc<GatewayService>,
    public: FQDN,
}

impl<'r> AsResponderTo<&'r AddrStream> for Bouncer {
    fn as_responder_to(&self, _req: &'r AddrStream) -> Self {
        self.clone()
    }
}

impl Bouncer {
    async fn bounce(self, req: Request<Body>) -> Result<Response, Error> {
        let mut resp = Response::builder();

        let host = req.headers().typed_get::<Host>().unwrap();
        let hostname = host.hostname();
        let fqdn = fqdn!(hostname);

        let path = req.uri();

        if fqdn.is_subdomain_of(&self.public)
            || self
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
}

impl Service<Request<Body>> for Bouncer {
    type Response = Response;
    type Error = Error;
    type Future =
        Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send + 'static>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        self.clone().bounce(req).boxed()
    }
}

pub struct UserServiceBuilder {
    service: Option<Arc<GatewayService>>,
    acme: Option<AcmeClient>,
    tls_acceptor: Option<RustlsAcceptor<DefaultAcceptor>>,
    bouncer_binds_to: Option<SocketAddr>,
    user_binds_to: Option<SocketAddr>,
    public: Option<FQDN>,
}

impl Default for UserServiceBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl UserServiceBuilder {
    pub fn new() -> Self {
        Self {
            service: None,
            public: None,
            acme: None,
            tls_acceptor: None,
            bouncer_binds_to: None,
            user_binds_to: None,
        }
    }

    pub fn with_public(mut self, public: FQDN) -> Self {
        self.public = Some(public);
        self
    }

    pub fn with_service(mut self, service: Arc<GatewayService>) -> Self {
        self.service = Some(service);
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
        let public = self.public.expect("a public FQDN is required");
        let user_binds_to = self
            .user_binds_to
            .expect("a socket address to bind to is required");

        let user_proxy = UserProxy {
            gateway: service.clone(),
            remote_addr: "127.0.0.1:80".parse().unwrap(),
            public: public.clone(),
        };

        let bouncer = self.bouncer_binds_to.as_ref().map(|_| Bouncer {
            gateway: service.clone(),
            public: public.clone(),
        });

        let mut futs = Vec::new();
        if let Some(tls_acceptor) = self.tls_acceptor {
            // TLS is enabled
            let bouncer = bouncer.expect("TLS cannot be enabled without a bouncer");
            let bouncer_binds_to = self.bouncer_binds_to.unwrap();

            let acme = self
                .acme
                .expect("TLS cannot be enabled without an ACME client");

            let bouncer = ServiceBuilder::new()
                .layer(ChallengeResponderLayer::new(acme))
                .service(bouncer);

            let bouncer = axum_server::Server::bind(bouncer_binds_to)
                .serve(bouncer.into_make_service())
                .map(|handle| ("bouncer (with challenge responder)", handle))
                .boxed();

            futs.push(bouncer);

            let user_with_tls = axum_server::Server::bind(user_binds_to)
                .acceptor(tls_acceptor)
                .serve(user_proxy.into_make_service())
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
                .serve(user_proxy.into_make_service())
                .map(|handle| ("user proxy (no TLS)", handle))
                .boxed();
            futs.push(user_without_tls);
        }

        future::select_all(futs.into_iter()).map(|((name, resolved), _, _)| {
            error!(service = %name, "exited early");
            resolved
        })
    }
}
