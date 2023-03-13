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
//!     let router = Router::with_path("hello").get(hello_world);
//!
//!     Ok(router.into())
//! }
//!
//! ```
use shuttle_runtime::Error;
use std::net::SocketAddr;

/// A wrapper type for [salvo::Router] so we can implement [shuttle_runtime::Service] for it.
pub struct SalvoService(pub salvo::Router);

#[shuttle_runtime::async_trait]
impl shuttle_runtime::Service for SalvoService {
    /// Takes the router that is returned by the user in their [shuttle_runtime::main] function
    /// and binds to an address passed in by shuttle.
    async fn bind(mut self, addr: SocketAddr) -> Result<(), Error> {
        salvo::Server::new(salvo::listener::TcpListener::bind(addr))
            .serve(self.0)
            .await;

        Ok(())
    }
}

impl From<salvo::Router> for SalvoService {
    fn from(router: salvo::Router) -> Self {
        Self(router)
    }
}
/// The return type that should be returned from the [shuttle_runtime::main] function.
pub type ShuttleSalvo = Result<SalvoService, Error>;
