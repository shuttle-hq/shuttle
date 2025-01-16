#![doc = include_str!("../README.md")]
use salvo::Listener;
use shuttle_runtime::Error;
use std::net::SocketAddr;

pub use salvo;

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

#[doc = include_str!("../README.md")]
pub type ShuttleSalvo = Result<SalvoService, Error>;
