#![doc = include_str!("../README.md")]
use salvo::Listener;
use std::net::SocketAddr;

pub use salvo;

/// A wrapper type for [salvo::Router] so we can implement [shuttle_runtime::Service] for it.
pub struct SalvoService(pub salvo::Router);

#[shuttle_runtime::async_trait]
impl shuttle_runtime::Service for SalvoService {
    async fn bind(mut self, addr: SocketAddr) -> Result<(), shuttle_runtime::BoxDynError> {
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
pub type ShuttleSalvo = Result<SalvoService, shuttle_runtime::BoxDynError>;
