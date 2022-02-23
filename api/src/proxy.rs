use std::future::Future;
use std::net::IpAddr;
use std::sync::Arc;
use hyper::server::conn::AddrStream;
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
        service_fn(async move |req: Request<Body>| { // returns BoxFut
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
            let (port, path) = match port {
                None => (api_port, "/404"),
                Some(port) => (port, req.uri().path())
            };

            // let's proxy
            reverse_proxy(
                remote_addr.ip(),
                port,
                path,
                req,
            ).await;
        })
    });

    let server = Server::bind(&socket_address)
        .serve(make_svc)
        .map_err(|e| eprintln!("server error: {}", e))
        .await;
}

async fn reverse_proxy(ip: IpAddr, port: Port, path: &str, req: Request<Body>) -> Result<Response<Body>, ProxyError> {
    let forward_uri = format!("http://127.0.0.1:{}{}", port, path);
    hyper_reverse_proxy::call(
        ip,
        &forward_uri,
        req).await
}