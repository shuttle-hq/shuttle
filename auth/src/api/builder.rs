use axum::{
    middleware::from_extractor,
    routing::{get, post},
    Router,
};
use shuttle_common::{
    backends::metrics::{Metrics, TraceLayer},
    request_span,
};
use sqlx::{Pool, Sqlite};
use tracing::field;

use crate::user::UserManager;

use super::handlers::{
    convert_cookie, convert_key, get_public_key, get_user, login, logout, post_user, refresh_token,
};

#[derive(Clone)]
pub(crate) struct RouterState {
    pub user_manager: UserManager,
}

pub(crate) struct ApiBuilder {
    router: Router<RouterState>,
    sqlite_pool: Option<Pool<Sqlite>>,
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
            .route("/user/:account_name", get(get_user).post(post_user))
            .route_layer(from_extractor::<Metrics>())
            .layer(
                TraceLayer::new(|request| {
                    request_span!(request, request.params.account_name = field::Empty)
                })
                .with_propagation()
                .build(),
            );

        Self {
            router,
            sqlite_pool: None,
        }
    }

    pub fn with_sqlite_pool(mut self, pool: Pool<Sqlite>) -> Self {
        self.sqlite_pool = Some(pool);
        self
    }

    pub fn into_router(self) -> Router {
        let pool = self.sqlite_pool.expect("an sqlite pool is required");

        let user_manager = UserManager { pool };
        self.router.with_state(RouterState { user_manager })
    }
}
