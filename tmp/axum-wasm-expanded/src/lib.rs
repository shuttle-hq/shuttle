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
        .route("/goodbye", axum::routing::get(goodbye))
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
pub extern "C" fn __SHUTTLE_Axum_call(
    fd_3: std::os::wasi::prelude::RawFd,
    fd_4: std::os::wasi::prelude::RawFd,
) {
    use axum::body::HttpBody;
    use std::io::{Read, Write};
    use std::os::wasi::io::FromRawFd;

    println!("inner handler awoken; interacting with fd={fd_3},{fd_4}");

    // file descriptor 3 for reading and writing http parts
    let mut parts_fd = unsafe { std::fs::File::from_raw_fd(fd_3) };

    let reader = std::io::BufReader::new(&mut parts_fd);

    // deserialize request parts from rust messagepack
    let wrapper: shuttle_common::wasm::RequestWrapper = rmp_serde::from_read(reader).unwrap();

    // file descriptor 4 for reading and writing http body
    let mut body_fd = unsafe { std::fs::File::from_raw_fd(fd_4) };

    // read body from host
    let mut body_buf = Vec::new();
    let mut c_buf: [u8; 1] = [0; 1];
    loop {
        body_fd.read(&mut c_buf).unwrap();
        if c_buf[0] == 0 {
            break;
        } else {
            body_buf.push(c_buf[0]);
        }
    }

    let request: http::Request<axum::body::Body> = wrapper
        .into_request_builder()
        .body(body_buf.into())
        .unwrap();

    println!("inner router received request: {:?}", &request);
    let res = handle_request(request);

    let (parts, mut body) = res.into_parts();

    // wrap and serialize response parts as rmp
    let response_parts = shuttle_common::wasm::ResponseWrapper::from(parts).into_rmp();

    // write response parts
    parts_fd.write_all(&response_parts).unwrap();

    // write body if there is one
    if let Some(body) = futures_executor::block_on(body.data()) {
        body_fd.write_all(body.unwrap().as_ref()).unwrap();
    }
    // signal to the reader that end of file has been reached
    body_fd.write(&[0]).unwrap();
}
