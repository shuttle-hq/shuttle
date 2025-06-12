#![doc = include_str!("../README.md")]
use std::net::SocketAddr;

#[cfg(feature = "axum")]
pub use axum;
#[cfg(feature = "axum-0-7")]
pub use axum_0_7 as axum;

#[cfg(feature = "axum")]
use axum::Router;
#[cfg(feature = "axum-0-7")]
use axum_0_7::Router;

/// A wrapper type for [axum::Router] so we can implement [shuttle_runtime::Service] for it.
pub struct AxumService(pub Router);

#[shuttle_runtime::async_trait]
impl shuttle_runtime::Service for AxumService {
    async fn bind(mut self, addr: SocketAddr) -> Result<(), shuttle_runtime::BoxDynError> {
        #[cfg(feature = "axum")]
        axum::serve(
            shuttle_runtime::tokio::net::TcpListener::bind(addr).await?,
            self.0,
        )
        .await?;
        #[cfg(feature = "axum-0-7")]
        axum_0_7::serve(
            shuttle_runtime::tokio::net::TcpListener::bind(addr).await?,
            self.0,
        )
        .await?;

        Ok(())
    }
}

impl From<Router> for AxumService {
    fn from(router: Router) -> Self {
        Self(router)
    }
}

#[doc = include_str!("../README.md")]
pub type ShuttleAxum = Result<AxumService, shuttle_runtime::BoxDynError>;
