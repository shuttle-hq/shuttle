use std::future::Future;
use async_trait::async_trait;

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
    async fn build(&mut self, _: &dyn Factory) -> Result<(), Error> {
        Ok(())
    }

    fn bind(
        &mut self,
        addr: SocketAddr,
    ) -> Pin<Box<dyn Future<Output = Result<(), Error>> + Send + Sync + 'static>>;
}

pub trait IntoService {
    type Service: Service;
    fn into_service(self) -> Self::Service;
}

pub struct RocketService(Option<Rocket<Build>>);

impl IntoService for Rocket<Build> {
    type Service = RocketService;
    fn into_service(self) -> Self::Service {
        RocketService(Some(self))
    }
}

#[async_trait]
impl Service for RocketService {
    fn bind(
        &mut self,
        addr: SocketAddr,
    ) -> Pin<Box<dyn Future<Output = Result<(), Error>> + Send + Sync + 'static>> {
        let rocket = self.0.take().expect("service has already been bound");
        Box::pin(async move {
            let runtime = Runtime::new()?;
            let config = rocket::Config {
                address: addr.ip(),
                port: addr.port(),
                log_level: rocket::config::LogLevel::Normal,
                ..Default::default()
            };
            let launched = rocket.configure(config).launch();
            runtime.block_on(launched)?;
            Ok(())
        })
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
}
