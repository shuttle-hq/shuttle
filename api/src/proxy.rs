use std::convert::Infallible;
use std::future::Future;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use hyper_reverse_proxy::ProxyError;
use ::hyper::server::{Server, conn::AddrStream};
use ::hyper::{Body, Request, Response, StatusCode};
use ::hyper::service::{service_fn, make_service_fn};
use ::hyper::http::uri::InvalidUri;
use lib::Port;

use crate::DeploymentSystem;
use crate::router::Router;

pub(crate) async fn start(proxy_port: Port, api_port: Port, deployment_manager: Arc<DeploymentSystem>) {
    let socket_address = ([127, 0, 0, 1], proxy_port).into();

    // A `Service` is needed for every connection.
    let make_svc = make_service_fn(|socket: &AddrStream| {
        let dm_ref = deployment_manager.clone();
        let remote_addr = socket.remote_addr();
        async move {
            Ok::<_, Infallible>(service_fn(move |req| handle(remote_addr, req, api_port, dm_ref.clone())))
        }
    });

    let server = Server::bind(&socket_address).serve(make_svc);
    if let Err(e) = server.await {
        eprintln!("server error: {}", e);
    }
    // todo, need to kill everything if proxy dies
}

async fn handle(
    remote_addr: SocketAddr,
    req: Request<Body>,
    api_port: Port,
    deployment_manager: Arc<DeploymentSystem>,
) -> Result<Response<Body>, Infallible> {
    // if no subdomain or `unveil.sh`, route to our API.
    let port = match req.headers().get("Host") {
        None => Some(api_port),
        Some(host) => {
            match host.to_str().unwrap() {
                "unveil.sh" => Some(api_port),
                host => deployment_manager.port_for_host(&String::from(host)).await
            }
        }
    };

    // if we could not get a port from the deployment manager,
    // the host does not exist so we use the api port and route to
    // the /404 endpoint
    let uri = req.uri().clone();
    let (port, path) = match port {
        None => (api_port, "/404"),
        Some(port) => (port, uri.path())
    };

    // let's proxy
    match reverse_proxy(
        remote_addr.ip(),
        port,
        path,
        req,
    ).await {
        Ok(response) => { Ok(response) }
        Err(error) => {
            match error {
                ProxyError::InvalidUri(e) => { dbg!("error while handling request in reverse proxy: {}", e); }
                ProxyError::HyperError(e) => { dbg!("error while handling request in reverse proxy: {}", e); }
                ProxyError::ForwardHeaderError => { dbg!("error while handling request in reverse proxy: 'fwd header error'"); }
            };
            Ok(
                Response::builder()
                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                    .body(Body::empty())
                    .unwrap()
            )
        }
    }
}

async fn reverse_proxy(ip: IpAddr, port: Port, path: &str, req: Request<Body>) -> Result<Response<Body>, ProxyError> {
    let forward_uri = format!("http://127.0.0.1:{}{}", port, path);
    hyper_reverse_proxy::call(
        ip,
        &forward_uri,
        req).await
}