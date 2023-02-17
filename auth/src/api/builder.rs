use std::net::SocketAddr;

use axum::{
    middleware::from_extractor,
    routing::{get, post},
    Router, Server,
};
use shuttle_common::{
    backends::metrics::{Metrics, TraceLayer},
    request_span,
};
use sqlx::SqlitePool;
use tracing::field;

use crate::user::UserManager;

use super::handlers::{
    convert_cookie, convert_key, get_public_key, get_user, login, logout, post_user, refresh_token,
};

#[derive(Clone)]
pub struct RouterState {
    pub user_manager: UserManager,
}

pub struct ApiBuilder {
    router: Router<RouterState>,
    pool: Option<SqlitePool>,
}

impl Default for ApiBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl ApiBuilder {
    pub fn new() -> Self {
        let router = Router::new()
            .route("/login", post(login))
            .route("/logout", post(logout))
            .route("/auth/session", post(convert_cookie))
            .route("/auth/key", post(convert_key))
            .route("/auth/refresh", post(refresh_token))
            .route("/public-key", get(get_public_key))
            .route("/user/:account_name", get(get_user))
            .route("/user/:account_name/:account_tier", post(post_user))
            .route_layer(from_extractor::<Metrics>())
            .layer(
                TraceLayer::new(|request| {
                    request_span!(request, request.params.account_name = field::Empty)
                })
                .with_propagation()
                .build(),
            );

        Self { router, pool: None }
    }

    pub fn with_sqlite_pool(mut self, pool: SqlitePool) -> Self {
        self.pool = Some(pool);
        self
    }

    pub fn into_router(self) -> Router {
        let pool = self.pool.expect("an sqlite pool is required");

        let user_manager = UserManager { pool };
        self.router.with_state(RouterState { user_manager })
    }
}

pub async fn serve(router: Router, address: SocketAddr) {
    Server::bind(&address)
        .serve(router.into_make_service())
        .await
        .unwrap_or_else(|_| panic!("Failed to bind to address: {}", address));
}
