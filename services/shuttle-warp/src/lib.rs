#![doc = include_str!("../README.md")]
use std::net::SocketAddr;
use std::ops::Deref;

pub use warp;

/// A wrapper type for [warp::Filter] so we can implement [shuttle_runtime::Service] for it.
pub struct WarpService<T>(pub T);

#[shuttle_runtime::async_trait]
impl<T> shuttle_runtime::Service for WarpService<T>
where
    T: Send + Sync + Clone + 'static + warp::Filter,
    T::Extract: warp::reply::Reply,
{
    async fn bind(mut self, addr: SocketAddr) -> Result<(), shuttle_runtime::BoxDynError> {
        warp::serve((*self).clone()).run(addr).await;
        Ok(())
    }
}

impl<T> From<T> for WarpService<T>
where
    T: Send + Sync + Clone + 'static + warp::Filter,
    T::Extract: warp::reply::Reply,
{
    fn from(router: T) -> Self {
        Self(router)
    }
}

impl<T> Deref for WarpService<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[doc = include_str!("../README.md")]
pub type ShuttleWarp<T> =
    Result<WarpService<warp::filters::BoxedFilter<T>>, shuttle_runtime::BoxDynError>;
