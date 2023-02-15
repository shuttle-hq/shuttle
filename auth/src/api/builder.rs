use std::str::FromStr;

use axum::{
    middleware::from_extractor,
    routing::{get, post},
    Router,
};
use shuttle_common::{
    backends::metrics::{Metrics, TraceLayer},
    request_span,
};
use sqlx::{
    migrate::Migrator,
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqliteSynchronous},
    Pool, Sqlite, SqlitePool,
};
use tracing::field;

use crate::user::UserManager;

use super::handlers::{
    convert_cookie, convert_key, get_public_key, get_user, login, logout, post_user, refresh_token,
};

pub static MIGRATIONS: Migrator = sqlx::migrate!("./migrations");

#[derive(Clone)]
pub(crate) struct RouterState {
    pub user_manager: UserManager,
}

pub struct ApiBuilder {
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

    pub async fn with_sqlite_pool(mut self, db_uri: &str) -> Self {
        // https://github.com/shuttle-hq/shuttle/pull/623
        let sqlite_options = SqliteConnectOptions::from_str(db_uri)
            .unwrap()
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(SqliteSynchronous::Normal);

        let pool = SqlitePool::connect_with(sqlite_options).await.unwrap();

        MIGRATIONS.run(&pool).await.unwrap();

        self.sqlite_pool = Some(pool);

        self
    }

    pub fn into_router(self) -> Router {
        let pool = self.sqlite_pool.expect("an sqlite pool is required");

        let user_manager = UserManager { pool };
        self.router.with_state(RouterState { user_manager })
    }
}
