#![doc = include_str!("../README.md")]
use std::net::SocketAddr;

pub use rocket;

/// A wrapper type for [rocket::Rocket<rocket::Build>] so we can implement [shuttle_runtime::Service] for it.
pub struct RocketService(pub rocket::Rocket<rocket::Build>);

#[shuttle_runtime::async_trait]
impl shuttle_runtime::Service for RocketService {
    async fn bind(mut self, addr: SocketAddr) -> Result<(), shuttle_runtime::BoxDynError> {
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

        let _rocket = self.0.configure(config).launch().await?;

        Ok(())
    }
}

impl From<rocket::Rocket<rocket::Build>> for RocketService {
    fn from(router: rocket::Rocket<rocket::Build>) -> Self {
        Self(router)
    }
}

#[doc = include_str!("../README.md")]
pub type ShuttleRocket = Result<RocketService, shuttle_runtime::BoxDynError>;
