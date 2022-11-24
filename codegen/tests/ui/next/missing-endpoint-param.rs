shuttle_codegen::app! {
    #[shuttle_codegen::endpoint(method = get)]
    async fn hello() -> &'static str {
        "Hello, World!"
    }

    #[shuttle_codegen::endpoint(route = "/goodbye")]
    async fn goodbye() -> &'static str {
        "Goodbye, World!"
    }

    #[shuttle_codegen::endpoint()]
    async fn goodbye() -> &'static str {
        "Goodbye, World!"
    }
}
