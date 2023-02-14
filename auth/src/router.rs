use axum::{
    routing::{get, post},
    Router,
};

pub fn new() -> Router {
    Router::new()
        .route("/login", post(login))
        .route("/logout", post(logout))
        .route("/auth/session", post(convert_cookie))
        .route("/auth/key", post(convert_key))
        .route("/auth/refresh", post(refresh_token))
        .route("/public-key", get(get_public_key))
        .route("/user/:account_name", get(get_user).post(post_user))
}

async fn login() {}

async fn logout() {}

async fn convert_cookie() {}

async fn convert_key() {}

async fn refresh_token() {}

async fn get_public_key() {}

async fn get_user() {}

async fn post_user() {}
