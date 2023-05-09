//! Shuttle service integration for the Salvo web framework.
//! ## Example
//! ```rust,no_run
//! use salvo::prelude::*;
//!
//! #[handler]
//! async fn hello_world(res: &mut Response) {
//!     res.render(Text::Plain("Hello, world!"));
//! }
//!
//! #[shuttle_runtime::main]
//! async fn salvo() -> shuttle_salvo::ShuttleSalvo {
//!     let router = Router::new().get(hello_world);
//!
//!     Ok(router.into())
//! }
//!
//! ```
use salvo::Listener;
use shuttle_runtime::Error;
use std::net::SocketAddr;

/// A wrapper type for [salvo::Router] so we can implement [shuttle_runtime::Service] for it.
pub struct SalvoService(pub salvo::Router);

#[shuttle_runtime::async_trait]
impl shuttle_runtime::Service for SalvoService {
    /// Takes the router that is returned by the user in their [shuttle_runtime::main] function
    /// and binds to an address passed in by shuttle.
    async fn bind(mut self, addr: SocketAddr) -> Result<(), Error> {
        let listener = salvo::conn::TcpListener::new(addr).bind().await;

        salvo::Server::new(listener).serve(self.0).await;

        Ok(())
    }
}

impl From<salvo::Router> for SalvoService {
    fn from(router: salvo::Router) -> Self {
        Self(router)
    }
}

/// Return type from the `[shuttle_runtime::main]` macro for a Salvo-based service.
/// 
/// # Example
/// 
/// ```rust,no_run
/// #[shuttle_runtime::main]
/// async example_service() ->
///    ShuttleSalvo<impl FnOnce(&mut ServiceConfig) + Send + Clone + 'static> {
///    todo!()
/// }
/// ```
pub type ShuttleSalvo = Result<SalvoService, Error>;
