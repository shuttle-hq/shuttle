#![doc = include_str!("../README.md")]
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
    async fn bind(mut self, addr: SocketAddr) -> Result<(), shuttle_runtime::BoxDynError> {
        let shared = tower::make::Shared::new(self.0);
        hyper::Server::bind(&addr).serve(shared).await?;

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
pub type ShuttleTower<T> = Result<TowerService<T>, shuttle_runtime::BoxDynError>;
