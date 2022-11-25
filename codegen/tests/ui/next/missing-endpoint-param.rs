shuttle_codegen::app! {
    #[shuttle_codegen::endpoint(method = get)]
    async fn only_method_param() -> &'static str {
        "Hello, World!"
    }

    #[shuttle_codegen::endpoint(route = "/goodbye")]
    async fn only_route_param() -> &'static str {
        "Goodbye, World!"
    }

    #[shuttle_codegen::endpoint()]
    async fn no_params() -> &'static str {
        "Goodbye, World!"
    }
}
