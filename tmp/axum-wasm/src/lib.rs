use tracing::debug;

shuttle_codegen::app! {
    #[shuttle_codegen::endpoint(method = get, route = "/hello")]
    async fn hello() -> &'static str {
        debug!("called hello()");
        "Hello, World!"
    }

    #[shuttle_codegen::endpoint(method = get, route = "/goodbye")]
    async fn goodbye() -> &'static str {
        debug!("called goodbye()");
        "Goodbye, World!"
    }
}
