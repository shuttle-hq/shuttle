use futures::TryStreamExt;
use shuttle_next::{
    body::BoxBody,
    extract::BodyStream,
    response::{IntoResponse, Response},
};
use tracing::debug;

pub fn handle_request(req: shuttle_next::Request<BoxBody>) -> shuttle_next::response::Response {
    shuttle_next::block_on(app(req))
}

async fn app(request: shuttle_next::Request<BoxBody>) -> shuttle_next::response::Response {
    use shuttle_next::Service;

    let mut router = shuttle_next::Router::new()
        .route("/hello", shuttle_next::routing::get(hello))
        .route("/goodbye", shuttle_next::routing::get(goodbye))
        .route("/uppercase", shuttle_next::routing::post(uppercase));

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
    Response::new(shuttle_next::body::StreamBody::new(chunk_stream))
}

#[no_mangle]
#[allow(non_snake_case)]
pub extern "C" fn __SHUTTLE_Axum_call(
    logs_fd: std::os::wasi::prelude::RawFd,
    parts_fd: std::os::wasi::prelude::RawFd,
    body_fd: std::os::wasi::prelude::RawFd,
) {
    use shuttle_next::body::{Body, HttpBody};
    use shuttle_next::tracing_prelude::*;
    use shuttle_next::Logger;
    use std::io::{Read, Write};
    use std::os::wasi::io::FromRawFd;

    // file descriptor 2 for writing logs to
    let logs_fd = unsafe { std::fs::File::from_raw_fd(logs_fd) };

    shuttle_next::tracing_registry()
        .with(Logger::new(logs_fd))
        .init(); // this sets the subscriber as the global default and also adds a compatibility layer for capturing `log::Record`s

    // file descriptor 3 for reading and writing http parts
    let mut parts_fd = unsafe { std::fs::File::from_raw_fd(parts_fd) };

    let reader = std::io::BufReader::new(&mut parts_fd);

    // deserialize request parts from rust messagepack
    let wrapper: shuttle_next::RequestWrapper = shuttle_next::from_read(reader).unwrap();

    // file descriptor 4 for reading and writing http body
    let mut body_stream = unsafe { std::fs::File::from_raw_fd(body_fd) };

    let mut reader = std::io::BufReader::new(&mut body_stream);
    let mut body_buf = Vec::new();
    reader.read_to_end(&mut body_buf).unwrap();

    let body = Body::from(body_buf);

    let request = wrapper
        .into_request_builder()
        .body(shuttle_next::body::boxed(body))
        .unwrap();

    let res = handle_request(request);

    let (parts, mut body) = res.into_parts();

    // wrap and serialize response parts as rmp
    let response_parts = shuttle_next::ResponseWrapper::from(parts)
        .into_rmp()
        .expect("failed to serialize response parts");

    // write response parts
    parts_fd.write_all(&response_parts).unwrap();

    // write body if there is one
    if let Some(body) = shuttle_next::block_on(body.data()) {
        body_stream.write_all(body.unwrap().as_ref()).unwrap();
    }
}
