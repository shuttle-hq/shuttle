use axum::{body::Body, response::Response, Router};
use hyper::http::{header::AUTHORIZATION, Request};
use shuttle_auth::{ApiBuilder, Sqlite};
use tower::ServiceExt;

pub(crate) const ADMIN_KEY: &str = "ndh9z58jttoes3qv";

pub(crate) struct TestApp {
    pub router: Router,
}

/// Initialize a router with an in-memory sqlite database for each test.
pub(crate) async fn app() -> TestApp {
    let sqlite = Sqlite::new_in_memory().await;

    // Insert an admin user for the tests.
    sqlite.insert_admin("admin", Some(ADMIN_KEY)).await;

    let router = ApiBuilder::new()
        .with_sqlite(sqlite)
        .with_sessions()
        .into_router();

    TestApp { router }
}

impl TestApp {
    pub async fn send_request(&self, request: Request<Body>) -> Response {
        self.router
            .clone()
            .oneshot(request)
            .await
            .expect("Failed to execute request.")
    }

    pub async fn post_user(&self, name: &str, tier: &str) -> Response {
        let request = Request::builder()
            .uri(format!("/users/{name}/{tier}"))
            .method("POST")
            .header(AUTHORIZATION, format!("Bearer {ADMIN_KEY}"))
            .body(Body::empty())
            .unwrap();

        self.send_request(request).await
    }

    pub async fn get_user(&self, name: &str) -> Response {
        let request = Request::builder()
            .uri(format!("/users/{name}"))
            .header(AUTHORIZATION, format!("Bearer {ADMIN_KEY}"))
            .body(Body::empty())
            .unwrap();

        self.send_request(request).await
    }
}
