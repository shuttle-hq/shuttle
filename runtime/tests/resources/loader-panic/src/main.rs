struct MyService;

#[shuttle_runtime::async_trait]
impl shuttle_runtime::Service for MyService {
    async fn bind(mut self, _: std::net::SocketAddr) -> Result<(), shuttle_runtime::Error> {
        Ok(())
    }
}

#[derive(Default)]
struct Thing;

#[shuttle_runtime::async_trait]
impl shuttle_service::ResourceInputBuilder for Thing {
    type Input = ();
    type Output = ();

    async fn build(
        self,
        _factory: &shuttle_service::ResourceFactory,
    ) -> Result<Self::Input, shuttle_service::Error> {
        panic!("panic in load");
    }
}

#[shuttle_runtime::main]
async fn load_panic(#[Thing] _a: ()) -> Result<MyService, shuttle_runtime::Error> {
    Ok(MyService)
}
