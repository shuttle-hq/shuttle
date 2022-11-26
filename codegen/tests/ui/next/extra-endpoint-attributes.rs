shuttle_codegen::app! {
    #[shuttle_codegen::endpoint(method = get, route = "/hello")]
    #[shuttle_codegen::endpoint(method = post, route = "/hello")]
    async fn hello() -> &'static str {
        "Hello, World!"
}}
