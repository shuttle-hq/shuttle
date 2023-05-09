//! Shuttle service integration for the Warp web framework.
//! ## Example
//! ```rust,no_run
//! use warp::Filter;
//! use warp::Reply;
//!
//! #[shuttle_runtime::main]
//! async fn warp() -> shuttle_warp::ShuttleWarp<(impl Reply,)> {
//!     let route = warp::any().map(|| "Hello, World!");
//!     Ok(route.boxed().into())
//! }
//! ```
use shuttle_runtime::Error;
use std::net::SocketAddr;
use std::ops::Deref;

/// A wrapper type for [warp::Filter] so we can implement [shuttle_runtime::Service] for it.
pub struct WarpService<T>(pub T);

#[shuttle_runtime::async_trait]
impl<T> shuttle_runtime::Service for WarpService<T>
where
    T: Send + Sync + Clone + 'static + warp::Filter,
    T::Extract: warp::reply::Reply,
{
    /// Takes the router that is returned by the user in their [shuttle_runtime::main] function
    /// and binds to an address passed in by shuttle.
    async fn bind(mut self, addr: SocketAddr) -> Result<(), Error> {
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

/// The return type of the [shuttle_runtime::main] function for the Warp service.
///
///  # Example
/// ```rust,no_run
///  [shuttle_runtime::main]
/// async fn example_service() ->
///    ShuttleWarp<impl FnOnce(&mut ServiceConfig) + Send + Clone + 'static> {
///   todo!()
/// }
/// ```
pub type ShuttleWarp<T> = Result<WarpService<warp::filters::BoxedFilter<T>>, Error>;
