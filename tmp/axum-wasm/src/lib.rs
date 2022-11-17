use axum::body::{Body, HttpBody};
use axum::{response::Response, routing::get, Router};
use futures_executor::block_on;
use http::Request;
use shuttle_axum_utils::{RequestWrapper, ResponseWrapper};
use std::fs::File;
use std::io::BufReader;
use std::io::{Read, Write};
use std::os::wasi::prelude::*;
use tower_service::Service;

extern crate rmp_serde as rmps;

pub fn handle_request<B>(req: Request<B>) -> Response
where
    B: HttpBody + Send + 'static,
{
    block_on(app(req))
}

async fn app<B>(request: Request<B>) -> Response
where
    B: HttpBody + Send + 'static,
{
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
pub extern "C" fn __SHUTTLE_Axum_call(fd_3: RawFd, fd_4: RawFd) {
    println!("inner handler awoken; interacting with fd={fd_3},{fd_4}");

    // file descriptor 3 for reading and writing http parts
    let mut parts_fd = unsafe { File::from_raw_fd(fd_3) };

    let reader = BufReader::new(&mut parts_fd);

    // deserialize request parts from rust messagepack
    let wrapper: RequestWrapper = rmps::from_read(reader).unwrap();

    // file descriptor 4 for reading and writing http body
    let mut body_fd = unsafe { File::from_raw_fd(fd_4) };

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

    let request: Request<Body> = wrapper
        .into_request_builder()
        .body(body_buf.into())
        .unwrap();

    println!("inner router received request: {:?}", &request);
    let res = handle_request(request);

    let (parts, mut body) = res.into_parts();

    // wrap and serialize response parts as rmp
    let response_parts = ResponseWrapper::from(parts).into_rmp();

    // write response parts
    parts_fd.write_all(&response_parts).unwrap();

    // write body if there is one
    if let Some(body) = block_on(body.data()) {
        body_fd.write_all(body.unwrap().as_ref()).unwrap();
    }
    // signal to the reader that end of file has been reached
    body_fd.write(&[0]).unwrap();
}
