use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

use axum::response::Response;
use http::StatusCode;
use pin_project::pin_project;

pub mod auth;
pub mod cache;
pub mod future;
pub mod headers;
pub mod metrics;
pub mod tracing;

/// Future for layers that might return a different status code
#[pin_project]
pub struct StatusCodeFuture<F> {
    #[pin]
    state: ResponseState<F>,
}

#[pin_project(project = ResponseStateProj)]
pub enum ResponseState<F> {
    Called {
        #[pin]
        inner: F,
    },
    Unauthorized,
    Forbidden,
    BadRequest,
}

impl<F, Error> Future for StatusCodeFuture<F>
where
    F: Future<Output = Result<axum::response::Response, Error>>,
{
    type Output = Result<Response, Error>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();

        match this.state.project() {
            ResponseStateProj::Called { inner } => inner.poll(cx),
            ResponseStateProj::Unauthorized => Poll::Ready(Ok(Response::builder()
                .status(StatusCode::UNAUTHORIZED)
                .body(Default::default())
                .unwrap())),
            ResponseStateProj::Forbidden => Poll::Ready(Ok(Response::builder()
                .status(StatusCode::FORBIDDEN)
                .body(Default::default())
                .unwrap())),
            ResponseStateProj::BadRequest => Poll::Ready(Ok(Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .body(Default::default())
                .unwrap())),
        }
    }
}
