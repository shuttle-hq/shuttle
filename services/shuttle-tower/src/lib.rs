//! Shuttle service integration for the Tower framework.
//! ## Example
//! ```rust,no_run
//! use std::convert::Infallible;
//! use std::future::Future;
//! use std::pin::Pin;
//! use std::task::{Context, Poll};
//!
//! #[derive(Clone)]
//! struct HelloWorld;
//!
//! impl tower::Service<hyper::Request<hyper::Body>> for HelloWorld {
//!     type Response = hyper::Response<hyper::Body>;
//!     type Error = Infallible;
//!     type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send + Sync>>;
//!
//!     fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
//!         Poll::Ready(Ok(()))
//!     }
//!
//!     fn call(&mut self, _req: hyper::Request<hyper::Body>) -> Self::Future {
//!         let body = hyper::Body::from("Hello, world!");
//!         let resp = hyper::Response::builder()
//!             .status(200)
//!             .body(body)
//!             .expect("Unable to create the `hyper::Response` object");
//!
//!         let fut = async { Ok(resp) };
//!
//!         Box::pin(fut)
//!     }
//! }
//!
//! #[shuttle_runtime::main]
//! async fn tower() -> shuttle_tower::ShuttleTower<HelloWorld> {
//!     let service = HelloWorld;
//!
//!     Ok(service.into())
//! }
//! ```
use shuttle_runtime::{CustomError, Error};
use std::net::SocketAddr;

/// A wrapper type for [tower::Service] so we can implement [shuttle_runtime::Service] for it.
pub struct TowerService<T>(pub T);

#[shuttle_runtime::async_trait]
impl<T> shuttle_runtime::Service for TowerService<T>
where
    T: tower::Service<hyper::Request<hyper::Body>, Response = hyper::Response<hyper::Body>>
        + Clone
        + Send
        + Sync
        + 'static,
    T::Error: std::error::Error + Send + Sync,
    T::Future: std::future::Future + Send + Sync,
{
    /// Takes the service that is returned by the user in their [shuttle_runtime::main] function
    /// and binds to an address passed in by shuttle.
    async fn bind(mut self, addr: SocketAddr) -> Result<(), Error> {
        let shared = tower::make::Shared::new(self.0);
        hyper::Server::bind(&addr)
            .serve(shared)
            .await
            .map_err(CustomError::new)?;

        Ok(())
    }
}

impl<T> From<T> for TowerService<T>
where
    T: tower::Service<hyper::Request<hyper::Body>, Response = hyper::Response<hyper::Body>>
        + Clone
        + Send
        + Sync
        + 'static,
    T::Error: std::error::Error + Send + Sync,
    T::Future: std::future::Future + Send + Sync,
{
    fn from(service: T) -> Self {
        Self(service)
    }
}

/// Shuttle service return type for the Tower framework.
/// ## Example
/// ```rust,no_run
/// # use std::convert::Infallible;
/// # use std::future::Future;
/// # use std::pin::Pin;
/// # use std::task::{Context, Poll};
/// # #[derive(Clone)]
/// # struct HelloWorld;
/// # impl tower::Service<hyper::Request<hyper::Body>> for HelloWorld {
/// #     type Response = hyper::Response<hyper::Body>;
/// #     type Error = Infallible;
/// #     type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send + Sync>>;
/// #     fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
/// #         Poll::Ready(Ok(()))
/// #     }
/// #     fn call(&mut self, _req: hyper::Request<hyper::Body>) -> Self::Future {
/// #         let body = hyper::Body::from("Hello, world!");
/// #         let resp = hyper::Response::builder()
/// #             .status(200)
/// #             .body(body)
/// #             .expect("Unable to create the `hyper::Response` object");
/// #         let fut = async { Ok(resp) };
/// #         Box::pin(fut)
/// #     }
/// # }
/// # #[shuttle_runtime::main]
/// async fn tower() -> shuttle_tower::ShuttleTower<HelloWorld> {
///     let service = HelloWorld;
///     Ok(service.into())
/// }
/// ```
pub type ShuttleTower<T> = Result<TowerService<T>, Error>;
