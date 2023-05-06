//! Shuttle service integration for the Actix Web framework.
//! ## Example
//! ```rust,no_run
//! use actix_web::{get, web::ServiceConfig};
//! use shuttle_actix_web::ShuttleActixWeb;
//!
//! #[get("/hello")]
//! async fn hello_world() -> &'static str {
//!     "Hello World!"
//! }
//!
//! #[shuttle_runtime::main]
//! async fn actix_web(
//! ) -> ShuttleActixWeb<impl FnOnce(&mut ServiceConfig) + Send + Clone + 'static> {
//!     let config = move |cfg: &mut ServiceConfig| {
//!         cfg.service(hello_world);
//!     };
//!
//!     Ok(config.into())
//! }
//! ```
use actix_web::{web, HttpResponse};
use std::net::SocketAddr;

/// A wrapper type for a closure that returns an [actix_web::web::ServiceConfig] so we can implement
/// [shuttle_runtime::Service] for it.
#[derive(Clone)]
pub struct ActixWebService<F>(pub F);

#[shuttle_runtime::async_trait]
impl<F> shuttle_runtime::Service for ActixWebService<F>
where
    F: FnOnce(&mut actix_web::web::ServiceConfig) + Send + Clone + 'static,
{
    async fn bind(mut self, addr: SocketAddr) -> Result<(), shuttle_runtime::Error> {
        // Start a worker for each cpu, but no more than 4.
        let worker_count = num_cpus::get().min(4);

        let server = actix_web::HttpServer::new(move || {
            actix_web::App::new()
                .configure(self.0.clone())
                .route("/healthz", web::get().to(HttpResponse::Ok))
        })
        .workers(worker_count)
        .bind(addr)?
        .run();

        server.await.map_err(shuttle_runtime::CustomError::new)?;

        Ok(())
    }
}

impl<F> From<F> for ActixWebService<F>
where
    F: FnOnce(&mut actix_web::web::ServiceConfig) + Send + Clone + 'static,
{
    fn from(service_config: F) -> Self {
        Self(service_config)
    }
}

/// The return type that should be returned from the [shuttle_runtime::main] function.
pub type ShuttleActixWeb<F> = Result<ActixWebService<F>, shuttle_runtime::Error>;
