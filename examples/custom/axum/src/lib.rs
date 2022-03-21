use async_trait::async_trait;
use axum::{routing::get, Router};
use shuttle_service::{rocket::tokio::runtime::Runtime, IntoService, Service};
use sync_wrapper::SyncWrapper;

#[macro_use]
extern crate shuttle_service;

struct Routes(Router);

async fn root() -> &'static str {
    "Hello, World!"
}

fn axum() -> Routes {
    let router = Router::new().route("/", get(root));

    Routes(router)
}

declare_service!(Routes, axum);

impl IntoService for Routes {
    type Service = CustomService;

    fn into_service(self) -> Self::Service {
        CustomService {
            router: Some(SyncWrapper::new(self.0)),
            runtime: Runtime::new().unwrap(),
        }
    }
}

struct CustomService {
    router: Option<SyncWrapper<Router>>,
    runtime: Runtime,
}

#[async_trait]
impl Service for CustomService {
    fn bind(&mut self, addr: std::net::SocketAddr) -> Result<(), shuttle_service::Error> {
        let router = self
            .router
            .take()
            .expect("service has already been bound")
            .into_inner();

        self.runtime
            .block_on(async {
                axum::Server::bind(&addr)
                    .serve(router.into_make_service())
                    .await
            })
            .unwrap();

        Ok(())
    }
}
