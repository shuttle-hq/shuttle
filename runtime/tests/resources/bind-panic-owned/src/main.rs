struct MyService;

#[shuttle_runtime::async_trait]
impl shuttle_runtime::Service for MyService {
    async fn bind(mut self, _: std::net::SocketAddr) -> Result<(), shuttle_runtime::Error> {
        panic!("panic in {}", "bind");
    }
}

#[shuttle_runtime::main]
async fn bind_panic_owned() -> Result<MyService, shuttle_runtime::Error> {
    Ok(MyService)
}
