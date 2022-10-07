use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use axum::body::HttpBody;
use axum::response::{IntoResponse, Response};
use futures::prelude::*;
use hyper::body::Body;
use hyper::client::connect::dns::GaiResolver;
use hyper::client::HttpConnector;
use hyper::server::conn::AddrStream;
use hyper::{Client, Request};
use hyper_reverse_proxy::ReverseProxy;
use once_cell::sync::Lazy;
use tower::Service;

use crate::service::GatewayService;
use crate::{Error, ErrorKind, ProjectName};

static PROXY_CLIENT: Lazy<ReverseProxy<HttpConnector<GaiResolver>>> =
    Lazy::new(|| ReverseProxy::new(Client::new()));

pub struct ProxyService {
    gateway: Arc<GatewayService>,
    remote_addr: SocketAddr,
    fqdn: String,
}

impl Service<Request<Body>> for ProxyService {
    type Response = Response;
    type Error = Error;
    type Future =
        Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send + 'static>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        let remote_addr = self.remote_addr.ip();
        let gateway = Arc::clone(&self.gateway);
        let fqdn = self.fqdn.clone();
        Box::pin(
            async move {
                let project_str = req
                    .headers()
                    .get("Host")
                    .map(|head| head.to_str().unwrap())
                    .and_then(|host| host.strip_suffix('.').unwrap_or(host).strip_suffix(&fqdn))
                    .ok_or_else(|| Error::from_kind(ErrorKind::ProjectNotFound))?;

                let project_name: ProjectName = project_str
                    .parse()
                    .map_err(|_| Error::from_kind(ErrorKind::InvalidProjectName))?;

                let project = gateway.find_project(&project_name).await?;

                let target_ip = project
                    .target_ip()?
                    .ok_or_else(|| Error::from_kind(ErrorKind::ProjectNotReady))?;

                let target_url = format!("http://{}:{}", target_ip, 8000);

                let proxy = PROXY_CLIENT
                    .call(remote_addr, &target_url, req)
                    .await
                    .map_err(|_| Error::from_kind(ErrorKind::ProjectUnavailable))?;

                let (parts, body) = proxy.into_parts();
                let body = <Body as HttpBody>::map_err(body, axum::Error::new).boxed_unsync();
                Ok(Response::from_parts(parts, body))
            }
            .or_else(|err: Error| future::ready(Ok(err.into_response()))),
        )
    }
}

pub struct MakeProxyService {
    gateway: Arc<GatewayService>,
    fqdn: String,
}

impl<'r> Service<&'r AddrStream> for MakeProxyService {
    type Response = ProxyService;
    type Error = Error;
    type Future =
        Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send + 'static>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, target: &'r AddrStream) -> Self::Future {
        let gateway = Arc::clone(&self.gateway);
        let remote_addr = target.remote_addr();
        let fqdn = self.fqdn.clone();
        Box::pin(async move {
            Ok(ProxyService {
                remote_addr,
                gateway,
                fqdn,
            })
        })
    }
}

pub fn make_proxy(gateway: Arc<GatewayService>, fqdn: String) -> MakeProxyService {
    MakeProxyService {
        gateway,
        fqdn: format!(".{fqdn}"),
    }
}
