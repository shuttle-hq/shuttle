shuttle_codegen::app! {
    #[shuttle_codegen::endpoint(method = get, route = "/goodbye", invalid = bad)]
    async fn goodbye() -> &'static str {
        "Goodbye, World!"
    }
}
