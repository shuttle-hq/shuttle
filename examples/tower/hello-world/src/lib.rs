use tower::Service;
use std::task::{Context, Poll};
use std::future::Future;
use std::pin::Pin;
use http::{Request, Response, StatusCode};

struct HelloWorld;

type T = Request<Vec<u8>>;
type R = Response<Vec<u8>>;
type E = http::Error;
type F = Pin<Box<dyn Future<Output = Result<R, E>>>>;

impl Service<T> for HelloWorld {
    type Response = R;
    type Error = E;
    type Future = F;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request<Vec<u8>>) -> Self::Future {
        let body: Vec<u8> = "hello, world!\n"
            .as_bytes()
            .to_owned();

        let resp = Response::builder()
            .status(StatusCode::OK)
            .body(body)
            .expect("Unable to create response object");

        let future = async {
            Ok(resp)
        };

        Box::pin(future)
    }
}

#[shuttle_service::main]
async fn tower() -> Result<Box<dyn Service<T, Response = R, Error = E, Future = F> + Send + Sync>, shuttle_service::Error> {
    Ok(Box::new(HelloWorld))
}
