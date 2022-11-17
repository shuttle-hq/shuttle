use std::convert::Infallible;
use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use axum::headers::{HeaderMapExt, Host};
use axum::response::{IntoResponse, Response};
use fqdn::{fqdn, Fqdn, FQDN};
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
use tower::{Layer, Service, ServiceBuilder};
use tracing::{debug, debug_span, field, trace};
use tracing_opentelemetry::OpenTelemetrySpanExt;

use crate::acme::{AcmeClient, ChallengeResponder, ChallengeResponderLayer, CustomDomain};
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
        let span = debug_span!("proxy", http.method = %req.method(), http.uri = %req.uri(), http.status_code = field::Empty, project = field::Empty);
        trace!(?req, "serving proxy request");

        let project_str = req
            .headers()
            .typed_get::<Host>()
            .map(|host| fqdn!(host.hostname()))
            .and_then(|fqdn| {
                if fqdn.is_subdomain_of(&self.public) && fqdn.depth() == 3 {
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

impl<R> AsResponderTo<R> for Bouncer {
    fn as_responder_to(&self, req: R) -> Self {
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
            || self.gateway
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

pub fn make_proxy(
    gateway: Arc<GatewayService>,
    acme: AcmeClient,
    public: FQDN,
) -> (
    ResponderMakeService<ChallengeResponder<Bouncer>>,
    ResponderMakeService<UserProxy>,
) {
    debug!("making proxy");

    let bouncer = ServiceBuilder::new()
        .layer(ChallengeResponderLayer::new(acme.clone()))
        .service(Bouncer {
            gateway: Arc::clone(&gateway),
            public: public.clone(),
        });

    let proxy = UserProxy {
        gateway,
        remote_addr: "127.0.0.1".parse().unwrap(),
        public,
    };

    (bouncer.into_make_service(), proxy.into_make_service())
}
