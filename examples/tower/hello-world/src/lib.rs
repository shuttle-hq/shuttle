use tower::Service;
use std::task::{Context, Poll};
use std::future::Future;
use std::collections::VecDeque;
use std::pin::Pin;
use hyper::{Request};

struct HelloWorld;

type T = http_body::Full<VecDeque<u8>>;
type R = http_body::Full<VecDeque<u8>>;
type E = Box<dyn std::error::Error>;
type F = Pin<Box<dyn Future<Output = Result<R, E>>>>;

impl Service<T> for HelloWorld {
    type Response = R;
    type Error = E;
    type Future = F;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: T) -> Self::Future {
        let body = VecDeque::from("hello, world!\n".to_string().into_bytes());

        let resp = http_body::Full::new(body);

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
