use std::{net::SocketAddr, str::FromStr};

use axum::{body::Body, extract::Path, response::Response, routing::get, Router};
use http::header::CONTENT_TYPE;
use hyper::{
    http::{header::AUTHORIZATION, Request},
    Server,
};
use serde_json::Value;
use shuttle_auth::{sqlite_init, ApiBuilder};
use sqlx::query;
use tower::ServiceExt;

use crate::stripe::MOCKED_SUBSCRIPTIONS;

pub(crate) const ADMIN_KEY: &str = "ndh9z58jttoes3qv";

pub(crate) struct TestApp {
    pub router: Router,
    pub mocked_stripe_server: MockedStripeServer,
}

/// Initialize a router with an in-memory sqlite database for each test.
pub(crate) async fn app() -> TestApp {
    let sqlite_pool = sqlite_init("sqlite::memory:").await;
    let mocked_stripe_server = MockedStripeServer::default();
    // Insert an admin user for the tests.
    query("INSERT INTO users (account_name, key, account_tier) VALUES (?1, ?2, ?3)")
        .bind("admin")
        .bind(ADMIN_KEY)
        .bind("admin")
        .execute(&sqlite_pool)
        .await
        .unwrap();

    let router = ApiBuilder::new()
        .with_sqlite_pool(sqlite_pool)
        .with_sessions()
        .into_router(stripe::Client::from_url(
            mocked_stripe_server.uri.to_string().as_str(),
            "",
        ));

    TestApp {
        router,
        mocked_stripe_server,
    }
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

    pub async fn put_user(
        &self,
        name: &str,
        tier: &str,
        checkout_session: &'static str,
    ) -> Response {
        let request = Request::builder()
            .uri(format!("/users/{name}/{tier}"))
            .method("PUT")
            .header(AUTHORIZATION, format!("Bearer {ADMIN_KEY}"))
            .header(CONTENT_TYPE, "application/json")
            .body(Body::from(checkout_session))
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

#[derive(Clone)]
pub(crate) struct MockedStripeServer {
    uri: http::Uri,
    router: Router,
}

impl MockedStripeServer {
    async fn subscription_retrieve_handler(
        Path(subscription_id): Path<String>,
    ) -> axum::response::Response<String> {
        let sessions = MOCKED_SUBSCRIPTIONS
            .iter()
            .filter(|sub| sub.contains(format!("\"id\": \"{}\"", subscription_id).as_str()))
            .map(|sub| serde_json::from_str(sub).unwrap())
            .collect::<Vec<Value>>();
        if sessions.len() == 1 {
            return Response::new(sessions[0].to_string());
        }

        Response::builder()
            .status(http::StatusCode::NOT_FOUND)
            .body("subscription id not found".to_string())
            .unwrap()
    }

    pub(crate) async fn serve(self) {
        let address = &SocketAddr::from_str(
            format!("{}:{}", self.uri.host().unwrap(), self.uri.port().unwrap()).as_str(),
        )
        .unwrap();
        println!("serving on: {}", address);
        Server::bind(address)
            .serve(self.router.into_make_service())
            .await
            .unwrap_or_else(|_| panic!("Failed to bind to address: {}", self.uri));
    }
}

impl Default for MockedStripeServer {
    fn default() -> MockedStripeServer {
        let router = Router::new().route(
            "/v1/subscriptions/:subscription_id",
            get(MockedStripeServer::subscription_retrieve_handler),
        );
        MockedStripeServer {
            uri: http::Uri::from_str(
                format!(
                    "http://127.0.0.1:{}",
                    portpicker::pick_unused_port().unwrap()
                )
                .as_str(),
            )
            .unwrap(),
            router,
        }
    }
}
