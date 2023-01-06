shuttle_codegen::app! {
    #[shuttle_codegen::endpoint(method = get, route = "/hello")]
    async fn hello() -> &'static str {
        "Hello, World!"
    }

    #[shuttle_codegen::endpoint(method = get, route = "/goodbye")]
    async fn goodbye() -> &'static str {
        "Goodbye, World!"
    }
}
