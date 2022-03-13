use async_trait::async_trait;
use std::future::Future;

pub use rocket;
use rocket::{Build, Rocket};

use tokio::runtime::Runtime;

use std::net::SocketAddr;
use std::pin::Pin;

mod error;

mod factory;

pub use error::Error;

pub use factory::Factory;

#[async_trait]
pub trait Service: Send + Sync {
    fn build(&mut self, _: &mut dyn Factory) -> Result<(), Error> {
        Ok(())
    }

    fn bind(&mut self, addr: SocketAddr) -> Result<(), error::Error>;
}

pub trait IntoService {
    type Service: Service;
    fn into_service(self) -> Self::Service;
}

pub struct RocketService<T: Sized> {
    rocket: Option<Rocket<Build>>,
    state_builder:
        Option<fn(&mut dyn Factory) -> Pin<Box<dyn Future<Output = Result<T, Error>> + Send + '_>>>,
    runtime: Runtime,
}

impl IntoService for Rocket<Build> {
    type Service = RocketService<()>;
    fn into_service(self) -> Self::Service {
        RocketService {
            rocket: Some(self),
            state_builder: None,
            runtime: Runtime::new().unwrap(),
        }
    }
}

impl<T: Send + Sync + 'static> IntoService
    for (
        Rocket<Build>,
        fn(&mut dyn Factory) -> Pin<Box<dyn Future<Output = Result<T, Error>> + Send + '_>>,
    )
{
    type Service = RocketService<T>;

    fn into_service(self) -> Self::Service {
        RocketService {
            rocket: Some(self.0),
            state_builder: Some(self.1),
            runtime: Runtime::new().unwrap(),
        }
    }
}

#[async_trait]
impl<T> Service for RocketService<T>
where
    T: Send + Sync + 'static,
{
    fn build(&mut self, factory: &mut dyn Factory) -> Result<(), Error> {
        if let Some(state_builder) = self.state_builder.take() {
            // We want to build any sqlx pools on the same runtime the client code will run on. Without this expect to get errors of no tokio reactor being present.
            let state = self.runtime.block_on(state_builder(factory))?;

            if let Some(rocket) = self.rocket.take() {
                self.rocket.replace(rocket.manage(state));
            }
        }

        Ok(())
    }

    fn bind(&mut self, addr: SocketAddr) -> Result<(), error::Error> {
        let rocket = self.rocket.take().expect("service has already been bound");

        let config = rocket::Config {
            address: addr.ip(),
            port: addr.port(),
            log_level: rocket::config::LogLevel::Normal,
            ..Default::default()
        };
        let launched = rocket.configure(config).launch();
        self.runtime.block_on(launched)?;
        Ok(())
    }
}

#[macro_export]
macro_rules! declare_service {
    ($service_type:ty, $constructor:path) => {
        #[no_mangle]
        pub extern "C" fn _create_service() -> *mut dyn $crate::Service {
            // Ensure constructor returns concrete type.
            let constructor: fn() -> $service_type = $constructor;

            let obj = $crate::IntoService::into_service(constructor());
            let boxed: Box<dyn $crate::Service> = Box::new(obj);
            Box::into_raw(boxed)
        }
    };
    ($service_type:ty, $constructor:path, $state_builder:path) => {
        #[no_mangle]
        pub extern "C" fn _create_service() -> *mut dyn $crate::Service {
            // Ensure constructor returns concrete type.
            let constructor: fn() -> $service_type = $constructor;

            // Ensure state builder is a function
            let state_builder: fn(
                &mut dyn $crate::Factory,
            ) -> std::pin::Pin<
                Box<dyn std::future::Future<Output = Result<_, $crate::Error>> + Send + '_>,
            > = |factory| Box::pin($state_builder(factory));

            let obj = $crate::IntoService::into_service((constructor(), state_builder));
            let boxed: Box<dyn $crate::Service> = Box::new(obj);
            Box::into_raw(boxed)
        }
    };
}
