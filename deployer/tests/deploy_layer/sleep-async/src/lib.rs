use std::time::Duration;

use shuttle_service::{IntoService, ServeHandle, Service};
use tokio::{runtime::Runtime, time::sleep};

#[macro_use]
extern crate shuttle_service;

struct Wait(u64);

struct SleepService {
    duration: u64,
    runtime: Runtime,
}

fn simple() -> Wait {
    Wait(4)
}

impl IntoService for Wait {
    type Service = SleepService;

    fn into_service(self) -> Self::Service {
        SleepService {
            duration: self.0,
            runtime: Runtime::new().unwrap(),
        }
    }
}

impl Service for SleepService {
    fn bind(
        &mut self,
        _: std::net::SocketAddr,
    ) -> Result<ServeHandle, shuttle_service::error::Error> {
        let duration = Duration::from_secs(self.duration);
        let handle = self.runtime.spawn(async move {
            sleep(duration).await;
            Ok(())
        });

        Ok(handle)
    }
}

declare_service!(Wait, simple);
