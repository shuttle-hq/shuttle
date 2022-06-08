use std::convert::Infallible;
use std::io;
use std::net::SocketAddr;
use std::sync::Arc;
use std::task::{Poll, Context};
use std::future::Future;
use std::pin::Pin;

use hyper::body::Body;
use hyper::server::conn::AddrStream;
use hyper::service::{
    make_service_fn,
    service_fn
};
use hyper::{
    Request,
    Response,
    StatusCode
};
use hyper_reverse_proxy::ProxyError;
use tower::Service;

use shuttle_common::DeploymentMeta;
use tower::MakeService;

use crate::service::GatewayService;
use crate::{Error, ProjectName, Refresh, ErrorKind};

const SHUTTLEAPP_SUFFIX: &'static str = ".shuttleapp.rs";

pub struct ProxyService {
    gateway: Arc<GatewayService>,
    remote_addr: SocketAddr
}

impl Service<Request<Body>> for ProxyService {
    type Response = Response<Body>;
    type Error = Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send + 'static>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        let remote_addr = self.remote_addr.ip().clone();
        let gateway = Arc::clone(&self.gateway);
        Box::pin(async move {
            let project_str = req
                .headers()
                .get("Host")
                .map(|head| head.to_str().unwrap())
                .and_then(|host| {
                    host.strip_suffix(".")
                        .unwrap_or(host)
                        .strip_suffix(SHUTTLEAPP_SUFFIX)
                })
                .unwrap();

            let project_name: ProjectName = project_str.parse().unwrap(); // TODO invalid project name
            let project = gateway.find_project(&project_name).await.unwrap(); // TODO project not found
            let target_ip = project.target_ip().unwrap().unwrap(); // TODO project not ready
            let target_url = format!("http://{}:{}", target_ip, 8000);
            let proxy = hyper_reverse_proxy::call(
                remote_addr,
                &target_url,
            req
            );
            Ok(proxy.await.unwrap())
        })
    }
}

pub struct MakeProxyService {
    gateway: Arc<GatewayService>,
}

impl<'r> Service<&'r AddrStream> for MakeProxyService {
    type Response = ProxyService;
    type Error = Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send + 'static>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, target: &'r AddrStream) -> Self::Future {
        let gateway = Arc::clone(&self.gateway);
        let remote_addr = target.remote_addr();
        Box::pin(async move {
            Ok(ProxyService {
                remote_addr,
                gateway 
            })
        })
    }
}

pub fn make_proxy(gateway: Arc<GatewayService>) -> MakeProxyService {
    MakeProxyService { gateway }
}
