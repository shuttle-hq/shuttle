#![doc = include_str!("../README.md")]
use shuttle_runtime::Error;
use std::net::SocketAddr;

pub use thruster;

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

#[doc = include_str!("../README.md")]
pub type ShuttleThruster<T> = Result<ThrusterService<T>, Error>;
