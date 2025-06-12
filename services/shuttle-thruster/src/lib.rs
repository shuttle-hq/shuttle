#![doc = include_str!("../README.md")]
use std::net::SocketAddr;

pub use thruster;

/// A wrapper type for [thruster::ThrusterServer] so we can implement [shuttle_runtime::Service] for it.
pub struct ThrusterService<T>(pub T);

#[shuttle_runtime::async_trait]
impl<T> shuttle_runtime::Service for ThrusterService<T>
where
    T: thruster::ThrusterServer + Send + 'static,
{
    async fn bind(mut self, addr: SocketAddr) -> Result<(), shuttle_runtime::BoxDynError> {
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

#[doc = include_str!("../README.md")]
pub type ShuttleThruster<T> = Result<ThrusterService<T>, shuttle_runtime::BoxDynError>;
