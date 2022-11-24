shuttle_codegen::app! {
    async fn hello() -> &'static str {
        "Hello, World!"
    }

    async fn goodbye() -> &'static str {
        "Goodbye, World!"
    }
}
