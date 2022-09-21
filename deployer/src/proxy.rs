use std::{
    convert::Infallible,
    net::{IpAddr, SocketAddr},
};

use async_trait::async_trait;
use hyper::{
    client::{connect::dns::GaiResolver, HttpConnector},
    header::{HeaderValue, HOST, SERVER},
    Body, Client, Request, Response, StatusCode,
};
use hyper_reverse_proxy::{ProxyError, ReverseProxy};
use once_cell::sync::Lazy;
use tracing::error;

static PROXY_CLIENT: Lazy<ReverseProxy<HttpConnector<GaiResolver>>> =
    Lazy::new(|| ReverseProxy::new(Client::new()));
static SERVER_HEADER: Lazy<HeaderValue> = Lazy::new(|| "shuttle.rs".parse().unwrap());

pub async fn handle(
    remote_address: SocketAddr,
    req: Request<Body>,
    address_getter: impl AddressGetter,
) -> Result<Response<Body>, Infallible> {
    let host = match req.headers().get(HOST) {
        Some(host) => host.to_str().unwrap_or_default().to_owned(),
        None => {
            return Ok(Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .body(Body::empty())
                .unwrap());
        }
    };

    let proxy_address = match address_getter.from_host(&host).await {
        Ok(Some(address)) => address,
        Ok(None) => {
            let response_body = format!("could not find service for host: {}", host);
            return Ok(Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(response_body.into())
                .unwrap());
        }
        Err(err) => {
            error!(error = %err, host, "proxy failed to find address for host");

            let response_body = format!("failed to find service for host: {}", host);
            return Ok(Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(response_body.into())
                .unwrap());
        }
    };

    match reverse_proxy(remote_address.ip(), &proxy_address.to_string(), req).await {
        Ok(response) => Ok(response),
        Err(error) => {
            match error {
                ProxyError::InvalidUri(e) => {
                    error!(error = %e, "error while handling request in reverse proxy");
                }
                ProxyError::HyperError(e) => {
                    error!(error = %e, "error while handling request in reverse proxy");
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
    async fn from_host(&self, host: &str) -> crate::handlers::Result<Option<SocketAddr>>;
}

async fn reverse_proxy(
    remote_ip: IpAddr,
    service_address: &str,
    req: Request<Body>,
) -> Result<Response<Body>, ProxyError> {
    let mut response = PROXY_CLIENT.call(remote_ip, service_address, req).await?;

    response.headers_mut().insert(SERVER, SERVER_HEADER.clone());

    Ok(response)
}
