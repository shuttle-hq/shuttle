#[macro_use]
extern crate tower_web;
use tower_web::{ServiceBuilder, response::DefaultSerializer, error::DefaultCatch, middleware::Identity};

#[derive(Clone, Debug)]
struct HelloWorld;

#[derive(Response)]
struct HelloResponse {
    message: &'static str,
}

impl_web! {
    impl HelloWorld {
        #[get("/")]
        #[content_type("json")]
        fn hello_world(&self) -> Result<HelloResponse, ()> {
            Ok(HelloResponse {
                message: "hello world",
            })
        }
    }
}

#[shuttle_service::main]
async fn tower() -> Result<ServiceBuilder<HelloWorld, DefaultSerializer, DefaultCatch, Identity>, shuttle_service::Error> {
    let service = ServiceBuilder::new()
        .resource(HelloWorld);

    Ok(service)
}
