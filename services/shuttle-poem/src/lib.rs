#![doc = include_str!("../README.md")]
/// A wrapper type for [poem::Endpoint] so we can implement [shuttle_runtime::Service] for it.
pub struct PoemService<T>(pub T);

#[cfg(feature = "poem-1")]
use poem_1 as poem;
#[cfg(feature = "poem-2")]
use poem_2 as poem;

#[shuttle_runtime::async_trait]
impl<T> shuttle_runtime::Service for PoemService<T>
where
    T: poem::Endpoint + Send + 'static,
{
    async fn bind(mut self, addr: std::net::SocketAddr) -> Result<(), shuttle_runtime::Error> {
        poem::Server::new(poem::listener::TcpListener::bind(addr))
            .run(self.0)
            .await
            .map_err(shuttle_runtime::CustomError::new)?;

        Ok(())
    }
}

impl<T> From<T> for PoemService<T>
where
    T: poem::Endpoint + Send + 'static,
{
    fn from(router: T) -> Self {
        Self(router)
    }
}

#[doc = include_str!("../README.md")]
pub type ShuttlePoem<T> = Result<PoemService<T>, shuttle_runtime::Error>;
