pub fn handle_request<B>(req: http::Request<B>) -> axum::response::Response
where
    B: axum::body::HttpBody + Send + 'static,
{
    futures_executor::block_on(app(req))
}

async fn app<B>(request: http::Request<B>) -> axum::response::Response
where
    B: axum::body::HttpBody + Send + 'static,
{
    use tower_service::Service;

    let mut router = axum::Router::new()
        .route("/hello", axum::routing::get(hello))
        .route("/goodbye", axum::routing::get(goodbye));

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
pub extern "C" fn __SHUTTLE_Axum_call(
    fd_3: std::os::wasi::prelude::RawFd,
    fd_4: std::os::wasi::prelude::RawFd,
    fd_5: std::os::wasi::prelude::RawFd,
) {
    use axum::body::{Body, HttpBody};
    use futures::stream::TryStreamExt;
    use std::io::{BufReader, Read, Write};
    use std::os::wasi::io::FromRawFd;
    // println!("inner handler awoken; interacting with fd={fd_3},{fd_4}");

    // file descriptor 3 for reading and writing http parts
    let mut parts_fd = unsafe { std::fs::File::from_raw_fd(fd_3) };

    let reader = std::io::BufReader::new(&mut parts_fd);

    // deserialize request parts from rust messagepack
    let wrapper: shuttle_common::wasm::RequestWrapper = rmp_serde::from_read(reader).unwrap();

    // file descriptor 4 for reading http body into wasm
    let body_read_stream = unsafe { std::fs::File::from_raw_fd(fd_4) };

    let reader = BufReader::new(body_read_stream);
    let stream = futures::stream::iter(reader.bytes()).try_chunks(2);
    let body = Body::wrap_stream(stream);

    let request: http::Request<axum::body::Body> =
        wrapper.into_request_builder().body(body).unwrap();

    // println!("inner router received request: {:?}", &request);
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
