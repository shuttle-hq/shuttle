use axum::body::Bytes;
use axum::{body::HttpBody, response::Response, routing::get, Router};
use futures_executor::block_on;
use http::Request;
use std::fs::File;
use std::io::{Read, Write};
use std::os::wasi::prelude::*;
use tower_service::Service;

pub fn handle_request(endpoint: String) -> Option<Bytes> {
    let request: Request<String> = Request::builder()
        .uri(format!("https://serverless.example/{}", endpoint.clone()))
        .body("Some body".to_string())
        .unwrap();

    let response = block_on(app(request));

    let response_body = block_on(response.into_body().data());

    if let Some(body) = response_body {
        Some(body.unwrap())
    } else {
        None
    }
}

async fn app(request: Request<String>) -> Response {
    let mut router = Router::new()
        .route("/hello", get(hello))
        .route("/goodbye", get(goodbye))
        .into_service();

    let response = router.call(request).await.unwrap();

    response
}

async fn hello() -> &'static str {
    "Hello, World!"
}

async fn goodbye() -> &'static str {
    "Goodbye, World!"
}

#[no_mangle]
#[allow(non_snake_case)]
pub extern "C" fn __SHUTTLE_Axum_call(fd: RawFd) {
    println!("inner handler awoken; interacting with fd={fd}");

    let mut f = unsafe { File::from_raw_fd(fd) };

    let mut buf = Vec::new();
    let mut c_buf = [0; 1];
    loop {
        f.read(&mut c_buf).unwrap();
        if c_buf[0] == 0 {
            break;
        } else {
            buf.push(c_buf[0]);
        }
    }

    let endpoint = String::from_utf8(buf).unwrap();

    println!("inner router called; GET /{endpoint}");
    let res = handle_request(endpoint);

    if let Some(bytes) = res {
        f.write_all(&bytes).unwrap();
    }
}
