shuttle_codegen::app! {
    #[shuttle_codegen::endpoint(method = pet, route = "/hello")]
    async fn hello() -> &'static str {
        "Hello, World!"
    }

    #[shuttle_codegen::endpoint(method =, route = "/hello")]
    async fn hello() -> &'static str {
        "Hello, World!"
    }
}
