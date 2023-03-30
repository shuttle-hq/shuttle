shuttle_next::app! {
    #[shuttle_next::endpoint(method = get, route = "/hello")]
    async fn hello() -> &'static str {
        shared::hello()
    }
}
