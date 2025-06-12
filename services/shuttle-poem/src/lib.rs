#![doc = include_str!("../README.md")]

pub use poem;

/// A wrapper type for [poem::Endpoint] so we can implement [shuttle_runtime::Service] for it.
pub struct PoemService<T>(pub T);

#[shuttle_runtime::async_trait]
impl<T> shuttle_runtime::Service for PoemService<T>
where
    T: poem::Endpoint + Send + 'static,
{
    async fn bind(
        mut self,
        addr: std::net::SocketAddr,
    ) -> Result<(), shuttle_runtime::BoxDynError> {
        poem::Server::new(poem::listener::TcpListener::bind(addr))
            .run(self.0)
            .await?;

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
pub type ShuttlePoem<T> = Result<PoemService<T>, shuttle_runtime::BoxDynError>;
