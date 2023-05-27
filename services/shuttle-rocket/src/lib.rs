//! Shuttle service integration for the Rocket web framework.
//! ## Example
//! ```rust,no_run
//! #[macro_use]
//! extern crate rocket;
//!
//! # fn main() {
//! #[get("/")]
//! fn index() -> &'static str {
//!     "Hello, world!"
//! }
//!
//! #[shuttle_runtime::main]
//! async fn rocket() -> shuttle_rocket::ShuttleRocket {
//!     let rocket = rocket::build().mount("/hello", routes![index]);
//!
//!     Ok(rocket.into())
//! }
//! # }
//! ```
use std::net::SocketAddr;

/// A wrapper type for [rocket::Rocket<rocket::Build>] so we can implement [shuttle_runtime::Service] for it.
pub struct RocketService(pub rocket::Rocket<rocket::Build>);

#[shuttle_runtime::async_trait]
impl shuttle_runtime::Service for RocketService {
    /// Takes the router that is returned by the user in their [shuttle_runtime::main] function
    /// and binds to an address passed in by shuttle.
    async fn bind(mut self, addr: SocketAddr) -> Result<(), shuttle_runtime::Error> {
        let shutdown = rocket::config::Shutdown {
            ctrlc: false,
            ..rocket::config::Shutdown::default()
        };

        let config = self
            .0
            .figment()
            .clone()
            .merge((rocket::Config::ADDRESS, addr.ip()))
            .merge((rocket::Config::PORT, addr.port()))
            .merge((rocket::Config::LOG_LEVEL, rocket::config::LogLevel::Off))
            .merge((rocket::Config::SHUTDOWN, shutdown));

        let _rocket = self
            .0
            .configure(config)
            .launch()
            .await
            .map_err(shuttle_runtime::CustomError::new)?;

        Ok(())
    }
}

impl From<rocket::Rocket<rocket::Build>> for RocketService {
    fn from(router: rocket::Rocket<rocket::Build>) -> Self {
        Self(router)
    }
}

/// Return type from the `[shuttle_runtime::main]` macro for a Rocket-based service.
///
/// # Example
///
/// ```rust,no_run
/// use shuttle_rocket::ShuttleRocket;
/// use rocket;
///
/// #[shuttle_runtime::main]
/// async fn example_service() ->
///   ShuttleRocket {
///   let router = rocket::build();
///  Ok(router.into())
/// }
/// ```
pub type ShuttleRocket = Result<RocketService, shuttle_runtime::Error>;
