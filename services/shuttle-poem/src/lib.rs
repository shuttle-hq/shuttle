//! Shuttle service integration for the Poem web framework.
//! ## Example
//! ```rust,no_run
//! use poem::{get, handler, Route};
//! use shuttle_poem::ShuttlePoem;
//!
//! #[handler]
//! fn hello_world() -> &'static str {
//!     "Hello, world!"
//! }
//!
//! #[shuttle_runtime::main]
//! async fn poem() -> ShuttlePoem<impl poem::Endpoint> {
//!     let app = Route::new().at("/hello", get(hello_world));
//!
//!     Ok(app.into())
//! }
//!
//! ```

/// A wrapper type for [poem::Endpoint] so we can implement [shuttle_runtime::Service] for it.
pub struct PoemService<T>(pub T);

#[poem::handler]
fn healthz() -> poem::http::StatusCode {
    poem::http::StatusCode::OK
}

#[shuttle_runtime::async_trait]
impl<T> shuttle_runtime::Service for PoemService<T>
where
    T: poem::Endpoint + Send + 'static,
{
    async fn bind(mut self, addr: std::net::SocketAddr) -> Result<(), shuttle_runtime::Error> {
        let app = poem::Route::new()
            .at("/", self.0)
            .at("/_shuttle/healthz", poem::get(healthz));

        poem::Server::new(poem::listener::TcpListener::bind(addr))
            .run(app)
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

/// The return type that should be returned from the [shuttle_runtime::main] function.
pub type ShuttlePoem<T> = Result<PoemService<T>, shuttle_runtime::Error>;
