use std::convert::Infallible;
use std::net::{Ipv4Addr, SocketAddr};
use std::time::Duration;

use anyhow::Result;
use http::{Method, Response, StatusCode};
use http_body_util::{BodyExt, Full};
use hyper_1::body;
use hyper_1::body::Bytes;
use hyper_1::server::conn::http1;
use hyper_1::service::service_fn;
use hyper_util::rt::TokioIo;
use shuttle_common::constants::SHUTTLE_CONSOLE_URL;
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio::time::sleep;
use tracing::{debug, trace};

/// Starts a hyper server to recieve the API key through callback from console.
/// note: server stays up until the program exits
pub async fn device_auth() -> Result<String> {
    let (tx, mut rx) = mpsc::channel::<String>(8);

    let console_url =
        std::env::var("SHUTTLE_CONSOLE_URL").unwrap_or(SHUTTLE_CONSOLE_URL.to_owned());

    let ip = Ipv4Addr::LOCALHOST;
    let port = portpicker::pick_unused_port()
        .expect("unable to find available port for CLI auth callback server");
    let addr = SocketAddr::from((ip, port));

    debug!(%addr, "Starting api key callback server");
    tokio::spawn(async move {
        let listener = TcpListener::bind(addr).await.unwrap();
        let tx = tx;

        loop {
            let (stream, addr) = listener.accept().await.unwrap();
            trace!(%addr, "Incoming connection");
            let io = TokioIo::new(stream);
            let tx = tx.clone();

            tokio::spawn(async move {
                if let Err(err) = http1::Builder::new()
                    .serve_connection(
                        io,
                        service_fn(|req| async { handler(tx.clone(), req).await }),
                    )
                    .await
                {
                    eprintln!("Error serving connection: {:?}", err);
                }
            });
        }
    });

    let url = &format!("{}device-auth?callbackPort={}", console_url, port);
    let _ = webbrowser::open(url);
    println!("Complete login in Shuttle Console to authenticate CLI.");
    println!("If your browser did not automatically open, go to {url}");

    let key = rx.recv().await.unwrap();
    debug!("Got API key from callback");

    // allow the hyper server response time to get sent back to the frontend
    // before we proceed and drop the channel
    sleep(Duration::from_millis(200)).await;

    Ok(key)
}

async fn handler(
    tx: mpsc::Sender<String>,
    req: http::Request<body::Incoming>,
) -> Result<Response<Full<Bytes>>, Infallible> {
    let uri = req.uri();
    let method = req.method();
    trace!(%uri, %method, "Incoming request");

    if *uri == *"/" && method == Method::POST {
        trace!("Parsing body");
        let body = String::from_utf8(req.into_body().collect().await.unwrap().to_bytes().to_vec())
            .expect("failed to parse callback request body as a string");
        tx.send(body).await.unwrap();

        trace!("Responding 200");
        Ok(Response::builder()
            .status(StatusCode::OK)
            // Console's callback request to localhost goes cross origin.
            // CORS headers needed for it to "see" the result of the request
            // and redirect the user.
            .header(
                "Access-Control-Allow-Origin",
                "*", // TODO?: use console_url
            )
            .header("Access-Control-Allow-Methods", "POST")
            .body(Full::default())
            .unwrap())
    } else {
        trace!("Responding 404");
        Ok(Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Full::default())
            .unwrap())
    }
}
