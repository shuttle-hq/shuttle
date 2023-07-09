use std::{
    convert::Infallible,
    net::{IpAddr, SocketAddr},
};

use async_trait::async_trait;
use axum::headers::{HeaderMapExt, Host};
use fqdn::{fqdn, FQDN};
use hyper::{
    client::{connect::dns::GaiResolver, HttpConnector},
    header::{HeaderValue, SERVER},
    Body, Client, Request, Response, StatusCode, Version,
};
use hyper_reverse_proxy::{ProxyError, ReverseProxy};
use once_cell::sync::Lazy;
use opentelemetry::global;
use opentelemetry_http::HeaderExtractor;
use shuttle_common::backends::headers::XShuttleProject;
use tracing::{error, field, instrument, trace, Span};
use tracing_opentelemetry::OpenTelemetrySpanExt;

static H1_PROXY_CLIENT: Lazy<ReverseProxy<HttpConnector<GaiResolver>>> =
    Lazy::new(|| ReverseProxy::new(Client::new()));

static H2_PROXY_CLIENT: Lazy<ReverseProxy<HttpConnector<GaiResolver>>> =
    Lazy::new(|| ReverseProxy::new(Client::builder().http2_only(true).build_http()));

static SERVER_HEADER: Lazy<HeaderValue> = Lazy::new(|| "shuttle.rs".parse().unwrap());

#[instrument(name = "proxy_request", skip(address_getter), fields(http.method = %req.method(), http.uri = %req.uri(), http.status_code = field::Empty, http.version = ?req.version(), service = field::Empty))]
pub async fn handle(
    remote_address: SocketAddr,
    fqdn: FQDN,
    req: Request<Body>,
    address_getter: impl AddressGetter,
) -> Result<Response<Body>, Infallible> {
    let span = Span::current();
    let parent_context = global::get_text_map_propagator(|propagator| {
        propagator.extract(&HeaderExtractor(req.headers()))
    });
    span.set_parent(parent_context);

    // First try to get the Host header. This header is not allowed in H2, so if we
    // can't find it we try to get the host subcomponent from the URI authority.
    let Some(host) = req
        .headers()
        .typed_get::<Host>()
        .map(|host| fqdn!(host.hostname()))
        .or_else(|| req.uri().host().map(|host| fqdn!(host))) else {
            trace!("proxy request has no host header or URI authority host subcomponent");
            return Ok(Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .body(Body::from("request has no host header or URI authority host subcomponent"))
                .unwrap());
        };

    // We only have one service per project, and its name coincides with that of the project.
    let Some(service) = req.headers().typed_get::<XShuttleProject>().map(|project| project.0) else {
        trace!("proxy request has no X-Shuttle-Project header");
        return Ok(Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body(Body::from("request has no X-Shuttle-Project header"))
            .unwrap());
    };

    // Record current service for tracing purposes
    span.record("service", &service);

    let proxy_address = match address_getter.get_address_for_service(&service).await {
        Ok(Some(address)) => address,
        Ok(None) => {
            trace!(?host, service, "service not found on this server");
            let response_body = format!("could not find service: {}", service);
            return Ok(Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(response_body.into())
                .unwrap());
        }
        Err(err) => {
            error!(error = %err, service, "proxy failed to find address for host");

            let response_body = format!("failed to find service for host: {}", host);
            return Ok(Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(response_body.into())
                .unwrap());
        }
    };

    let client = match req.version() {
        Version::HTTP_10 | Version::HTTP_11 => &H1_PROXY_CLIENT,
        Version::HTTP_2 => &H2_PROXY_CLIENT,
        protocol => {
            error!(protocol = ?protocol, "received request with unsupported protocol");
            return Ok(Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::empty())
                .unwrap());
        }
    };

    match reverse_proxy(remote_address.ip(), &proxy_address.to_string(), req, client).await {
        Ok(response) => {
            Span::current().record("http.status_code", response.status().as_u16());
            Ok(response)
        }
        Err(error) => {
            match error {
                ProxyError::InvalidUri(e) => {
                    error!(error = %e, "error while handling request in reverse proxy: 'invalid uri'");
                }
                ProxyError::HyperError(e) => {
                    error!(error = %e, "error while handling request in reverse proxy: 'hyper error'");
                }
                ProxyError::ForwardHeaderError => {
                    error!("error while handling request in reverse proxy: 'fwd header error'");
                }
                ProxyError::UpgradeError(e) => error!(error = %e,
                    "error while handling request needing upgrade in reverse proxy"
                ),
            };
            Ok(Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::empty())
                .unwrap())
        }
    }
}

#[async_trait]
pub trait AddressGetter: Clone + Send + Sync + 'static {
    async fn get_address_for_service(
        &self,
        service_name: &str,
    ) -> crate::handlers::Result<Option<SocketAddr>>;
}

#[instrument(skip(req, client))]
async fn reverse_proxy(
    remote_ip: IpAddr,
    service_address: &str,
    req: Request<Body>,
    client: &Lazy<ReverseProxy<HttpConnector<GaiResolver>>>,
) -> Result<Response<Body>, ProxyError> {
    let forward_uri = format!("http://{service_address}");

    let mut response = client.call(remote_ip, &forward_uri, req).await?;

    response.headers_mut().insert(SERVER, SERVER_HEADER.clone());

    Ok(response)
}
