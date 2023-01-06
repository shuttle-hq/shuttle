use axum::{
    body::BoxBody,
    extract::BodyStream,
    response::{IntoResponse, Response},
};
use futures::TryStreamExt;
use tracing::debug;

pub fn handle_request(req: http::Request<BoxBody>) -> axum::response::Response {
    futures_executor::block_on(app(req))
}

async fn app(request: http::Request<BoxBody>) -> axum::response::Response {
    use tower_service::Service;

    let mut router = axum::Router::new()
        .route("/hello", axum::routing::get(hello))
        .route("/goodbye", axum::routing::get(goodbye))
        .route("/uppercase", axum::routing::post(uppercase));

    let response = router.call(request).await.unwrap();

    response
}

async fn hello() -> &'static str {
    debug!("in hello()");
    "Hello, World!"
}

async fn goodbye() -> &'static str {
    debug!("in goodbye()");
    "Goodbye, World!"
}

// Map the bytes of the body stream to uppercase and return the stream directly.
async fn uppercase(body: BodyStream) -> impl IntoResponse {
    debug!("in uppercase()");
    let chunk_stream = body.map_ok(|chunk| {
        chunk
            .iter()
            .map(|byte| byte.to_ascii_uppercase())
            .collect::<Vec<u8>>()
    });
    Response::new(axum::body::StreamBody::new(chunk_stream))
}

#[no_mangle]
#[allow(non_snake_case)]
pub extern "C" fn __SHUTTLE_Axum_call(
    fd_2: std::os::wasi::prelude::RawFd,
    fd_3: std::os::wasi::prelude::RawFd,
    fd_4: std::os::wasi::prelude::RawFd,
    fd_5: std::os::wasi::prelude::RawFd,
) {
    use axum::body::HttpBody;
    use shuttle_common::wasm::Logger;
    use std::io::{Read, Write};
    use std::os::wasi::io::FromRawFd;
    use tracing_subscriber::prelude::*;

    println!("inner handler awoken; interacting with fd={fd_2},{fd_3},{fd_4},{fd_5}");

    // file descriptor 2 for writing logs to
    let logs_fd = unsafe { std::fs::File::from_raw_fd(fd_2) };

    tracing_subscriber::registry()
        .with(Logger::new(logs_fd))
        .init(); // this sets the subscriber as the global default and also adds a compatibility layer for capturing `log::Record`s

    // file descriptor 3 for reading and writing http parts
    let mut parts_fd = unsafe { std::fs::File::from_raw_fd(fd_3) };

    let reader = std::io::BufReader::new(&mut parts_fd);

    // deserialize request parts from rust messagepack
    let wrapper: shuttle_common::wasm::RequestWrapper = rmp_serde::from_read(reader).unwrap();

    // file descriptor 4 for reading http body into wasm
    let mut body_read_stream = unsafe { std::fs::File::from_raw_fd(fd_4) };

    let mut reader = std::io::BufReader::new(&mut body_read_stream);
    let mut body_buf = Vec::new();
    reader.read_to_end(&mut body_buf).unwrap();

    let body = axum::body::Body::from(body_buf);

    let request = wrapper
        .into_request_builder()
        .body(axum::body::boxed(body))
        .unwrap();

    println!("inner router received request: {:?}", &request);
    let res = handle_request(request);

    let (parts, mut body) = res.into_parts();

    // wrap and serialize response parts as rmp
    let response_parts = shuttle_common::wasm::ResponseWrapper::from(parts).into_rmp();

    // write response parts
    parts_fd.write_all(&response_parts).unwrap();

    // file descriptor 5 for writing http body to host
    let mut body_write_stream = unsafe { std::fs::File::from_raw_fd(fd_5) };

    // write body if there is one
    if let Some(body) = futures_executor::block_on(body.data()) {
        body_write_stream.write_all(body.unwrap().as_ref()).unwrap();
    }
}
