use axum::{body::Body, response::Response, Router};
use hyper::http::{header::AUTHORIZATION, Request};
use shuttle_auth::{sqlite_init, ApiBuilder};
use sqlx::query;
use tower::ServiceExt;

pub(crate) const ADMIN_KEY: &str = "my-api-key";

pub struct TestApp {
    pub router: Router,
}

/// Initialize a router with an in-memory sqlite database for each test.
pub async fn app() -> TestApp {
    let sqlite_pool = sqlite_init("sqlite::memory:").await;

    query("INSERT INTO users (account_name, key, account_tier) VALUES (?1, ?2, ?3)")
        .bind("admin")
        .bind(ADMIN_KEY)
        .bind("admin")
        .execute(&sqlite_pool)
        .await
        .unwrap();

    let router = ApiBuilder::new()
        .with_sqlite_pool(sqlite_pool)
        .await
        .into_router();

    // Give the test-app time to start
    // tokio::time::sleep(Duration::from_millis(500)).await;

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

    pub async fn post_user(&self, name: &str) -> Response {
        let request = Request::builder()
            .uri(format!("/user/{name}"))
            .method("POST")
            .header(AUTHORIZATION, format!("Bearer {ADMIN_KEY}"))
            .body(Body::empty())
            .unwrap();

        self.send_request(request).await
    }

    pub async fn get_user(&self, name: &str) -> Response {
        let request = Request::builder()
            .uri(format!("/user/{name}"))
            .header(AUTHORIZATION, format!("Bearer {ADMIN_KEY}"))
            .body(Body::empty())
            .unwrap();

        self.send_request(request).await
    }
}
