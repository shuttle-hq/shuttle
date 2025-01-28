#![doc = include_str!("../README.md")]
use shuttle_runtime::{CustomError, Error};
use std::net::SocketAddr;

pub use tower;

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

#[doc = include_str!("../README.md")]
pub type ShuttleTower<T> = Result<TowerService<T>, Error>;
