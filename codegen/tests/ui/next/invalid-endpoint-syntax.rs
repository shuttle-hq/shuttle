shuttle_codegen::app! {
    #[shuttle_codegen::endpoint(method = get, route = "/hello" extra = abundant)]
    async fn hello() -> &'static str {
        "Hello, World!"
    }

    #[shuttle_codegen::endpoint(method = get, route = "/goodbye", invalid)]
    async fn goodbye() -> &'static str {
        "Goodbye, World!"
    }
}
