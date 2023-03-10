//! Shuttle service integration for the Axum web framework.
//! ## Example
//! ```rust,no_run
//! use shuttle_axum::AxumService;
//!
//! async fn hello_world() -> &'static str {
//!     "Hello, world!"
//! }
//!
//! #[shuttle_axum::main]
//! async fn axum() -> shuttle_service::ShuttleAxum {
//!     let router = Router::new().route("/hello", get(hello_world));
//!
//!     Ok(AxumService(router))
//! }
//! ```

/// A wrapper type for `axum::Router` so we can implement `shuttle_runtime::Service` for it.
pub struct AxumService(pub axum::Router);

#[shuttle_runtime::async_trait]
impl shuttle_runtime::Service for AxumService {
    /// Takes the router that is returned by the user in their `shuttle_runtime::main` function
    /// and binds to an address passed in by shuttle.
    async fn bind(mut self, addr: std::net::SocketAddr) -> Result<(), shuttle_runtime::Error> {
        axum::Server::bind(&addr)
            .serve(self.0.into_make_service())
            .await
            .map_err(shuttle_runtime::CustomError::new)?;

        Ok(())
    }
}

impl From<axum::Router> for AxumService {
    fn from(router: axum::Router) -> Self {
        Self(router)
    }
}
/// The return type that should be returned from the `shuttle_runtime::main` function.
pub type ShuttleAxum = Result<AxumService, shuttle_runtime::Error>;
