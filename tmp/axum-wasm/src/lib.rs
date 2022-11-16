use axum::body::HttpBody;
use axum::{response::Response, routing::get, Router};
use futures_executor::block_on;
use http::Request;
use shuttle_axum_utils::{RequestWrapper, ResponseWrapper};
use std::fs::File;
use std::io::{Read, Write};
use std::os::wasi::prelude::*;
use tower_service::Service;

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
pub extern "C" fn __SHUTTLE_Axum_call(fd: RawFd) {
    println!("inner handler awoken; interacting with fd={fd}");

    let mut f = unsafe { File::from_raw_fd(fd) };

    // read request parts from host
    let mut req_buf = Vec::new();
    let mut c_buf: [u8; 1] = [0; 1];
    loop {
        f.read(&mut c_buf).unwrap();
        if c_buf[0] == 0 {
            break;
        } else {
            req_buf.push(c_buf[0]);
        }
    }

    // deserialize request parts from rust messagepack
    let wrapper = RequestWrapper::from_rmp(req_buf);

    // TODO: deduplicate this? Is it the correct strategy to send two separate files?
    // read request body from host
    let mut body_buf = Vec::new();
    let mut c_buf: [u8; 1] = [0; 1];
    loop {
        f.read(&mut c_buf).unwrap();
        if c_buf[0] == 0 {
            break;
        } else {
            body_buf.push(c_buf[0]);
        }
    }

    // set body in the wrapper (Body::Empty if buf is empty), consume wrapper and return Request<Body>
    let request = wrapper.set_body(body_buf).into_request();

    println!("inner router received request: {:?}", &request);
    let res = handle_request(request);

    let (parts, mut body) = res.into_parts();

    println!("sending parts: {:?}", parts.headers.clone());
    // wrap and serialize response parts as rmp
    let response_parts = ResponseWrapper::from(parts).into_rmp();

    println!("sending response parts: {:?}", &response_parts);
    // write response parts
    f.write_all(&response_parts).unwrap();
    f.write(&[0]).unwrap();

    // write body
    f.write_all(block_on(body.data()).unwrap().unwrap().as_ref())
        .unwrap();
    f.write(&[0]).unwrap();
}
