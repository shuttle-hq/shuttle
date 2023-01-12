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

#[cfg(test)]
mod tests {
    use crate::__app;
    use http::Request;
    use hyper::Method;

    #[tokio::test]
    async fn hello() {
        let request = Request::builder()
            .uri("http://local.test/hello")
            .method(Method::GET)
            .body(axum::body::boxed(axum::body::Body::empty()))
            .unwrap();

        let response = __app(request).await;

        assert!(response.status().is_success());

        let body = &hyper::body::to_bytes(response.into_body()).await.unwrap();

        assert_eq!(body, "Hello, World!");
    }
}
