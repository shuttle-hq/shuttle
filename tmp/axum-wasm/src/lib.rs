use axum::{response::Response, routing::get, Router};
use futures_executor::block_on;
use http::Request;
use shuttle_axum_utils::{RequestWrapper, ResponseWrapper};
use std::fs::File;
use std::io::{Read, Write};
use std::os::wasi::prelude::*;
use tower_service::Service;

pub fn handle_request(req: Request<String>) -> Response {
    block_on(app(req))
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

    let req = RequestWrapper::from_rmp(req_buf);

    // todo: clean up conversion of wrapper to request
    let mut request: Request<String> = Request::builder()
        .method(req.method)
        .version(req.version)
        .uri(req.uri)
        .body("Some body".to_string())
        .unwrap();

    request.headers_mut().extend(req.headers.into_iter());

    println!("inner router received request: {:?}", &request);
    let res = handle_request(request);

    println!("inner router sending response: {:?}", &res);
    let response = ResponseWrapper::from(res);

    f.write_all(&response.into_rmp()).unwrap();
}
