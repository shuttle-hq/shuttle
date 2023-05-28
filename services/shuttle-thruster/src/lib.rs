//! Shuttle service integration for the Thruster web framework.
//! ## Example
//! ```rust,no_run
//! use thruster::{
//!     context::basic_hyper_context::{generate_context, BasicHyperContext as Ctx, HyperRequest},
//!     m, middleware_fn, App, HyperServer, MiddlewareNext, MiddlewareResult, ThrusterServer,
//! };
//!
//! #[middleware_fn]
//! async fn hello(mut context: Ctx, _next: MiddlewareNext<Ctx>) -> MiddlewareResult<Ctx> {
//!     context.body("Hello, World!");
//!     Ok(context)
//! }
//!
//! #[shuttle_runtime::main]
//! async fn thruster() -> shuttle_thruster::ShuttleThruster<HyperServer<Ctx, ()>> {
//!     let server = HyperServer::new(
//!         App::<HyperRequest, Ctx, ()>::create(generate_context, ()).get("/hello", m![hello]),
//!     );
//!     
//!     Ok(server.into())
//! }
//! ```
use shuttle_runtime::Error;
use std::net::SocketAddr;

/// A wrapper type for [thruster::ThrusterServer] so we can implement [shuttle_runtime::Service] for it.
pub struct ThrusterService<T>(pub T);

#[shuttle_runtime::async_trait]
impl<T> shuttle_runtime::Service for ThrusterService<T>
where
    T: thruster::ThrusterServer + Send + 'static,
{
    /// Takes the server that is returned by the user in their [shuttle_runtime::main] function
    /// and binds to an address passed in by shuttle.
    async fn bind(mut self, addr: SocketAddr) -> Result<(), Error> {
        self.0.build(&addr.ip().to_string(), addr.port()).await;

        Ok(())
    }
}

impl<T> From<T> for ThrusterService<T>
where
    T: thruster::ThrusterServer + Send + 'static,
{
    fn from(router: T) -> Self {
        Self(router)
    }
}

/// The return type of the [shuttle_runtime::main] function for the Thruster service.
///
/// # Example
/// ```rust,no_run
/// use shuttle_thruster::ShuttleThruster;
/// use thruster::{
///     context::basic_hyper_context::{generate_context, BasicHyperContext as Ctx, HyperRequest},
///     m, middleware_fn, App, HyperServer, MiddlewareNext, MiddlewareResult, ThrusterServer,
/// };
///
/// #[middleware_fn]
/// async fn hello(mut context: Ctx, _next: MiddlewareNext<Ctx>) -> MiddlewareResult<Ctx> {
///     context.body("Hello, World!");
///     Ok(context)
/// }
///
/// #[shuttle_runtime::main]
/// async fn thruster() -> ShuttleThruster<HyperServer<Ctx, ()>> {
///     Ok(HyperServer::new(
///         App::<HyperRequest, Ctx, ()>::create(generate_context, ()).get("/hello", m![hello]),
///     ).into())
/// }
///
/// ```
pub type ShuttleThruster<T> = Result<ThrusterService<T>, Error>;
