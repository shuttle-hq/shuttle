#![doc = include_str!("../README.md")]
use std::net::SocketAddr;

pub use tide;

/// A wrapper type for [tide::Server<T>] so we can implement [shuttle_runtime::Service] for it.
pub struct TideService<T>(pub tide::Server<T>);

#[shuttle_runtime::async_trait]
impl<T> shuttle_runtime::Service for TideService<T>
where
    T: Clone + Send + Sync + 'static,
{
    async fn bind(mut self, addr: SocketAddr) -> Result<(), shuttle_runtime::BoxDynError> {
        self.0.listen(addr).await?;

        Ok(())
    }
}

impl<T> From<tide::Server<T>> for TideService<T> {
    fn from(router: tide::Server<T>) -> Self {
        Self(router)
    }
}

#[doc = include_str!("../README.md")]
pub type ShuttleTide<T> = Result<TideService<T>, shuttle_runtime::BoxDynError>;
