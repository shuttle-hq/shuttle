use std::convert::Infallible;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use hyper_reverse_proxy::ProxyError;
use ::hyper::server::{Server, conn::AddrStream};
use ::hyper::{Body, Request, Response, StatusCode};
use ::hyper::service::{service_fn, make_service_fn};
use lib::Port;

use crate::DeploymentSystem;

pub(crate) async fn start(
    bind_addr: IpAddr,
    proxy_port: Port,
    deployment_manager: Arc<DeploymentSystem>
) {
    let socket_address = (bind_addr, proxy_port).into();

    // A `Service` is needed for every connection.
    let make_svc = make_service_fn(|socket: &AddrStream| {
        let dm_ref = deployment_manager.clone();
        let remote_addr = socket.remote_addr();
        async move {
            Ok::<_, Infallible>(service_fn(move |req| handle(remote_addr, req, dm_ref.clone())))
        }
    });

    let server = Server::bind(&socket_address).serve(make_svc);

    dbg!("starting proxy server: {}", &socket_address);

    if let Err(e) = server.await {
        eprintln!("server error: {}", e);
        eprintln!("proxy died, killing process...");
        std::process::exit(1);
    }
}

async fn handle(
    remote_addr: SocketAddr,
    req: Request<Body>,
    deployment_manager: Arc<DeploymentSystem>,
) -> Result<Response<Body>, Infallible> {
    // if no `Host:` or invalid value, return 400
    let host = match req.headers().get("Host") {
        Some(host) if host.to_str().is_ok() => host.to_str().unwrap().to_owned(),
        _ => return Ok(Response::builder().status(StatusCode::BAD_REQUEST).body(Body::empty()).unwrap())
    };

    // if we could not get a port from the deployment manager,
    // the host does not exist or is not initialised yet - so
    // we return a 404
    let port = match deployment_manager.port_for_host(&host).await {
        None => {
            // no port being assigned here means that we couldn't
            // find a service for a given host
            let response_body = format!("could not find service for host: {}", host);
            return Ok(
                Response::builder()
                    .status(StatusCode::NOT_FOUND)
                    .body(response_body.into())
                    .unwrap()
            );
        }
        Some(port) => port
    };
    
    match reverse_proxy(
        remote_addr.ip(),
        port,
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

async fn reverse_proxy(ip: IpAddr, port: Port, req: Request<Body>) -> Result<Response<Body>, ProxyError> {
    let forward_uri = format!("http://127.0.0.1:{}", port);
    hyper_reverse_proxy::call(
        ip,
        &forward_uri,
        req).await
}