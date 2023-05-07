//! Shuttle service integration for the Tide web framework.
//! ## Example
//! ```rust,no_run
//! #[shuttle_runtime::main]
//! async fn tide() -> shuttle_tide::ShuttleTide<()> {
//!     let mut app = tide::new();
//!     app.with(tide::log::LogMiddleware::new());
//!
//!     app.at("/hello").get(|_| async { Ok("Hello, world!") });
//!
//!     Ok(app.into())
//! }
//! ```
use shuttle_runtime::{CustomError, Error};
use std::net::SocketAddr;

/// A wrapper type for [tide::Server<T] so we can implement [shuttle_runtime::Service] for it.
pub struct TideService<T>(pub tide::Server<T>);

#[shuttle_runtime::async_trait]
impl<T> shuttle_runtime::Service for TideService<T>
where
    T: Clone + Send + Sync + 'static,
{
    /// Takes the router that is returned by the user in their [shuttle_runtime::main] function
    /// and binds to an address passed in by shuttle.
    async fn bind(mut self, addr: SocketAddr) -> Result<(), Error> {
        self.0.listen(addr).await.map_err(CustomError::new)?;

        Ok(())
    }
}

impl<T> From<tide::Server<T>> for TideService<T> {
    fn from(router: tide::Server<T>) -> Self {
        Self(router)
    }
}

/// Return type from the `[shuttle_runtime::main]` macro for a Tide-based service.
///
/// # Example
///
/// ```
/// #[shuttle_runtime::main]
/// async fn example_service()
///  -> ShuttleTide<impl FnOnce(&mut ServiceConfig) + Send + Clone + 'static> {
///     todo!();    
/// }
/// ```
pub type ShuttleTide<T> = Result<TideService<T>, Error>;
