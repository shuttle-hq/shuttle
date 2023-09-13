//! Shuttle service integration for the Axum web framework.
//! ## Example
//! ```rust,no_run
//! use axum::{routing::get, Router};
//!
//! async fn hello_world() -> &'static str {
//!     "Hello, world!"
//! }
//!
//! #[shuttle_runtime::main]
//! async fn axum() -> shuttle_axum::ShuttleAxum {
//!     let router = Router::new().route("/hello", get(hello_world));
//!
//!     Ok(router.into())
//! }
//! ```
use shuttle_runtime::{CustomError, Error};
use std::net::SocketAddr;

/// A wrapper type for [axum::Router] so we can implement [shuttle_runtime::Service] for it.
pub struct AxumService<S = ()>(pub axum::Router<S>);

#[shuttle_runtime::async_trait]
impl shuttle_runtime::Service for AxumService {
    /// Takes the router that is returned by the user in their [shuttle_runtime::main] function
    /// and binds to an address passed in by shuttle.
    async fn bind(mut self, addr: SocketAddr) -> Result<(), Error> {
        axum::Server::bind(&addr)
            .serve(self.0.into_make_service())
            .await
            .map_err(CustomError::new)?;

        Ok(())
    }
}

impl<S> From<axum::Router<S>> for AxumService<S> {
    fn from(router: axum::Router<S>) -> Self {
        Self(router)
    }
}

/// The return type that should be returned from the [shuttle_runtime::main] function.
pub type ShuttleAxum = Result<AxumService, Error>;
