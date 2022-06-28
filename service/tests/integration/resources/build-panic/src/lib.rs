use async_trait::async_trait;

use shuttle_service::{Error, Factory, IntoService, Runtime, ServeHandle, Service};

#[macro_use]
extern crate shuttle_service;

#[derive(Default)]
struct Builder;

impl IntoService for Builder {
    type Service = MyService;

    fn into_service(self) -> Self::Service {
        MyService {
            runtime: Runtime::new().unwrap(),
        }
    }
}

struct MyService {
    runtime: Runtime,
}

#[async_trait]
impl Service for MyService {
    async fn build(
        &mut self,
        _factory: &mut dyn Factory,
        _logger: Box<dyn log::Log>,
    ) -> Result<(), Error> {
        panic!("panic in build");
    }

    fn bind(
        &mut self,
        _: std::net::SocketAddr,
    ) -> Result<ServeHandle, shuttle_service::error::Error> {
        let handle = self.runtime.spawn(async move { Ok(()) });

        Ok(handle)
    }
}

declare_service!(Builder, Builder::default);
