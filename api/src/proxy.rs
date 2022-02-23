use std::convert::Infallible;
use std::future::Future;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use hyper_reverse_proxy::ProxyError;
use rocket::futures::TryFutureExt;
use rocket::http::hyper;
use rocket::http::hyper::{Body, HeaderValue, make_service_fn, Request, Response, Server, service_fn};
use lib::Port;
use crate::DeploymentSystem;

use crate::router::Router;

async fn start_proxy(proxy_port: Port, api_port: Port, deployment_manager: Arc<DeploymentSystem>) {
    let socket_address = ([127, 0, 0, 1], proxy_port).into();

    // A `Service` is needed for every connection.
    let make_svc = make_service_fn(|socket: &AddrStream| {
        let remote_addr = socket.remote_addr();
        async move {
            service_fn(move |req| handle(remote_addr, req, api_port, deployment_manager))
        }
    });

    let server = Server::bind(&socket_address)
        .serve(make_svc)
        .map_err(|e| eprintln!("server error: {}", e))
        .await;
}

async fn handle(
    remote_addr: SocketAddr,
    req: Request<Body>,
    api_port: Port,
    deployment_manager: Arc<DeploymentSystem>,
) -> Result<Response<Body>, ProxyError> {
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
    reverse_proxy(
        remote_addr.ip(),
        port,
        path,
        req,
    ).await
}

async fn reverse_proxy(ip: IpAddr, port: Port, path: &str, req: Request<Body>) -> Result<Response<Body>, ProxyError> {
    let forward_uri = format!("http://127.0.0.1:{}{}", port, path);
    hyper_reverse_proxy::call(
        ip,
        &forward_uri,
        req).await
}