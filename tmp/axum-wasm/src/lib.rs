use axum::body::HttpBody;
use axum::{response::Response, routing::get, Router};
use futures_executor::block_on;
use http::Request;
use shuttle_axum_utils::{wrap_response, RequestWrapper};
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

    // deserialize request from rust messagepack
    let req = RequestWrapper::from_rmp(req_buf);

    // consume wrapper and return Request
    let request = req.into_request();

    println!("inner router received request: {:?}", &request);
    let res = handle_request(request);

    println!("inner router sending response: {:?}", &res);
    // wrap inner response and serialize it as rust messagepack
    let response = block_on(wrap_response(res)).into_rmp();

    f.write_all(&response).unwrap();
}
