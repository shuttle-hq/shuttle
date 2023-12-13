use crate::stripe::MOCKED_SUBSCRIPTIONS;
use axum::{body::Body, extract::Path, extract::State, response::Response, routing::get, Router};
use http::{header::CONTENT_TYPE, StatusCode};
use hyper::{
    http::{header::AUTHORIZATION, Request},
    Server,
};
use once_cell::sync::Lazy;
use serde_json::Value;
use shuttle_auth::{pgpool_init, ApiBuilder};
use shuttle_common::claims::{AccountTier, Claim};
use shuttle_common_tests::postgres::DockerInstance;
use sqlx::query;
use std::{
    net::SocketAddr,
    str::FromStr,
    sync::{Arc, Mutex},
};
use tower::ServiceExt;

pub(crate) const ADMIN_KEY: &str = "ndh9z58jttoes3qv";

static PG: Lazy<DockerInstance> = Lazy::new(DockerInstance::default);
#[ctor::dtor]
fn cleanup() {
    PG.cleanup();
}

pub(crate) struct TestApp {
    pub router: Router,
    pub mocked_stripe_server: MockedStripeServer,
}

/// Initialize a router with an in-memory sqlite database for each test.
pub(crate) async fn app() -> TestApp {
    let pg_pool = pgpool_init(PG.get_unique_uri().as_str()).await.unwrap();

    let mocked_stripe_server = MockedStripeServer::default();
    // Insert an admin user for the tests.
    query("INSERT INTO users (account_name, key, account_tier) VALUES ($1, $2, $3)")
        .bind("admin")
        .bind(ADMIN_KEY)
        .bind(AccountTier::Admin.to_string())
        .execute(&pg_pool)
        .await
        .unwrap();

    let router = ApiBuilder::new()
        .with_pg_pool(pg_pool)
        .with_sessions()
        .with_stripe_client(stripe::Client::from_url(
            mocked_stripe_server.uri.to_string().as_str(),
            "",
        ))
        .with_jwt_signing_private_key("LS0tLS1CRUdJTiBQUklWQVRFIEtFWS0tLS0tCk1DNENBUUF3QlFZREsyVndCQ0lFSUR5V0ZFYzhKYm05NnA0ZGNLTEwvQWNvVUVsbUF0MVVKSTU4WTc4d1FpWk4KLS0tLS1FTkQgUFJJVkFURSBLRVktLS0tLQo=".to_string())
        .into_router();

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

    pub async fn get_jwt_from_api_key(&self, api_key: &str) -> Response {
        let request = Request::builder()
            .uri("/auth/key")
            .header(AUTHORIZATION, format!("Bearer {api_key}"))
            .body(Body::empty())
            .unwrap();
        self.send_request(request).await
    }

    pub async fn claim_from_response(&self, res: Response) -> Claim {
        let body = hyper::body::to_bytes(res.into_body()).await.unwrap();
        let convert: Value = serde_json::from_slice(&body).unwrap();
        let token = convert["token"].as_str().unwrap();

        let request = Request::builder()
            .uri("/public-key")
            .method("GET")
            .body(Body::empty())
            .unwrap();

        let response = self.send_request(request).await;

        assert_eq!(response.status(), StatusCode::OK);

        let public_key = hyper::body::to_bytes(response.into_body()).await.unwrap();

        Claim::from_token(token, &public_key).unwrap()
    }
}

#[derive(Clone)]
pub(crate) struct MockedStripeServer {
    uri: http::Uri,
    router: Router,
}

#[derive(Clone)]
pub(crate) struct RouterState {
    subscription_cancel_side_effect_toggle: Arc<Mutex<bool>>,
}

impl MockedStripeServer {
    async fn subscription_retrieve_handler(
        Path(subscription_id): Path<String>,
        State(state): State<RouterState>,
    ) -> axum::response::Response<String> {
        let is_sub_cancelled = state
            .subscription_cancel_side_effect_toggle
            .lock()
            .unwrap()
            .to_owned();

        if subscription_id == "sub_123" {
            if is_sub_cancelled {
                return Response::new(MOCKED_SUBSCRIPTIONS[3].to_string());
            } else {
                let mut toggle = state.subscription_cancel_side_effect_toggle.lock().unwrap();
                *toggle = true;
                return Response::new(MOCKED_SUBSCRIPTIONS[2].to_string());
            }
        }

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
        let router_state = RouterState {
            subscription_cancel_side_effect_toggle: Arc::new(Mutex::new(false)),
        };

        let router = Router::new()
            .route(
                "/v1/subscriptions/:subscription_id",
                get(MockedStripeServer::subscription_retrieve_handler),
            )
            .with_state(router_state);

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
