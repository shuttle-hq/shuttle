#![doc = include_str!("../README.md")]
use shuttle_runtime::{CustomError, Error};
use std::net::SocketAddr;

/// A wrapper type for [axum::Router] so we can implement [shuttle_runtime::Service] for it.
pub struct AxumService(pub axum::Router);

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

impl From<axum::Router> for AxumService {
    fn from(router: axum::Router) -> Self {
        Self(router)
    }
}

#[doc = include_str!("../README.md")]
pub type ShuttleAxum = Result<AxumService, Error>;
