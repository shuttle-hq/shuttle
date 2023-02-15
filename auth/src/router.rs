use axum::{
    middleware::from_extractor,
    routing::{get, post},
    Router,
};
use shuttle_common::backends::metrics::{Metrics, TraceLayer};
use tracing::field;

pub fn new() -> Router {
    Router::new()
        .route("/login", post(login))
        .route("/logout", post(logout))
        .route("/auth/session", post(convert_cookie))
        .route("/auth/key", post(convert_key))
        .route("/auth/refresh", post(refresh_token))
        .route("/public-key", get(get_public_key))
        .route("/user/:account_name", get(get_user).post(post_user))
        .route_layer(from_extractor::<Metrics>())
        .layer(
            TraceLayer::new()
                .extra_fields(|_| vec![("request.params.account_name", Box::new(field::Empty))])
                .with_propagation()
                .build(),
        )
}

async fn login() {}

async fn logout() {}

async fn convert_cookie() {}

async fn convert_key() {}

async fn refresh_token() {}

async fn get_public_key() {}

async fn get_user() {}

async fn post_user() {}
