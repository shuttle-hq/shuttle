use shuttle_service::Service;

struct MyService;

#[shuttle_service::async_trait]
impl Service for MyService {
    async fn bind(
        mut self: Box<Self>,
        _: std::net::SocketAddr,
    ) -> Result<(), shuttle_service::Error> {
        Ok(())
    }
}

#[shuttle_service::main]
async fn build_panic() -> Result<MyService, shuttle_service::Error> {
    panic!("panic in build");
}
