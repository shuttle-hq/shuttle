use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

use axum::response::Response;
use http::StatusCode;
use pin_project::pin_project;

// Future for layers that just return the inner response
#[pin_project]
pub struct ResponseFuture<F>(#[pin] pub F);

impl<F, Response, Error> Future for ResponseFuture<F>
where
    F: Future<Output = Result<Response, Error>>,
{
    type Output = Result<Response, Error>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();

        this.0.poll(cx)
    }
}

/// Future for layers that might return a different status code
#[pin_project(project = StatusCodeProj)]
pub enum StatusCodeFuture<F> {
    // A future that should be polled
    Poll(#[pin] F),

    // A status code to return
    Code(StatusCode),
}

impl<F, Error> Future for StatusCodeFuture<F>
where
    F: Future<Output = Result<axum::response::Response, Error>>,
{
    type Output = Result<Response, Error>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();

        match this {
            StatusCodeProj::Poll(inner) => inner.poll(cx),
            StatusCodeProj::Code(status_code) => Poll::Ready(Ok(Response::builder()
                .status(*status_code)
                .body(Default::default())
                .unwrap())),
        }
    }
}
