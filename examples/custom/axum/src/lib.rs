use async_trait::async_trait;
use axum::{routing::get, Router};
use shuttle_service::{rocket::tokio::runtime::Runtime, IntoService, Service};

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

declare_service!(Routes<_>, axum);

impl IntoService for Routes {
    type Service = CustomService;

    fn into_service(self) -> Self::Service {
        CustomService {
            router: Some(self.0),
            runtime: Runtime::new().unwrap(),
        }
    }
}

struct CustomService {
    router: Option<Router>,
    runtime: Runtime,
}

#[async_trait]
impl Service for CustomService {
    fn bind(&mut self, addr: std::net::SocketAddr) -> Result<(), shuttle_service::Error> {
        let (path, handler) = self.router.take().expect("service has already been bound");
        let app = Router::new().route(&path, get(handler));

        let launched = axum::Server::bind(&addr).serve(app.into_make_service());
        self.runtime.block_on(launched).unwrap();

        Ok(())
    }
}
