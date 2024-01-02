use std::{net::SocketAddr, sync::Arc};

use axum::{
    extract::FromRef,
    handler::Handler,
    middleware::from_extractor,
    routing::{delete, get, post, put},
    Router, Server,
};
use axum_sessions::{async_session::MemoryStore, SessionLayer};
use rand::RngCore;
use shuttle_common::{
    backends::{
        auth::{JwtAuthenticationLayer, ScopedLayer},
        metrics::{Metrics, TraceLayer},
    },
    claims::Scope,
    request_span,
};
use sqlx::PgPool;
use tracing::field;

use crate::{
    secrets::{EdDsaManager, KeyManager},
    user::{UserManagement, UserManager},
    COOKIE_EXPIRATION,
};

use super::handlers::{
    add_subscription_items, convert_cookie, convert_key, delete_subscription_items, get_public_key,
    get_user, health_check, logout, post_user, put_user_reset_key, refresh_token, update_user_tier,
};

pub type UserManagerState = Arc<Box<dyn UserManagement>>;
pub type KeyManagerState = Arc<Box<dyn KeyManager>>;

#[derive(Clone)]
pub struct RouterState {
    pub user_manager: UserManagerState,
    pub key_manager: KeyManagerState,
    pub rds_price_id: String,
}

// Allow getting a user management state directly
impl FromRef<RouterState> for UserManagerState {
    fn from_ref(router_state: &RouterState) -> Self {
        router_state.user_manager.clone()
    }
}

// Allow getting a key manager state directly
impl FromRef<RouterState> for KeyManagerState {
    fn from_ref(router_state: &RouterState) -> Self {
        router_state.key_manager.clone()
    }
}

pub struct ApiBuilder {
    router: Router<RouterState>,
    pool: Option<PgPool>,
    session_layer: Option<SessionLayer<MemoryStore>>,
    stripe_client: Option<stripe::Client>,
    rds_price_id: Option<String>,
    key_manager: EdDsaManager,
}

impl ApiBuilder {
    pub fn new(jwt_signing_private_key: String) -> Self {
        let key_manager = EdDsaManager::new(jwt_signing_private_key);

        let public_key = key_manager.public_key().to_vec();

        // A separate router for subscription routes, guarded by the JWT auth layer.
        let subscription_routes = Router::new()
            .route(
                "/items",
                post(add_subscription_items.layer(ScopedLayer::new(vec![Scope::ResourcesWrite]))),
            )
            .route(
                "/items/:metadata_id",
                delete(
                    delete_subscription_items.layer(ScopedLayer::new(vec![Scope::ResourcesWrite])),
                ),
            )
            .layer(JwtAuthenticationLayer::new(move || {
                let public_key = public_key.clone();
                async move { public_key.clone() }
            }));

        let router = Router::new()
            .route("/", get(health_check))
            .route("/logout", post(logout))
            .route("/auth/session", get(convert_cookie))
            .route("/auth/key", get(convert_key))
            .route("/auth/refresh", post(refresh_token))
            .route("/public-key", get(get_public_key))
            .route("/users/:account_name", get(get_user))
            .route(
                "/users/:account_name/:account_tier",
                post(post_user).put(update_user_tier),
            )
            .route("/users/reset-api-key", put(put_user_reset_key))
            .nest("/users/subscription", subscription_routes)
            .route_layer(from_extractor::<Metrics>())
            .layer(
                TraceLayer::new(|request| {
                    request_span!(
                        request,
                        request.params.account_name = field::Empty,
                        request.params.account_tier = field::Empty
                    )
                })
                .with_propagation()
                .build(),
            );

        Self {
            router,
            pool: None,
            session_layer: None,
            stripe_client: None,
            rds_price_id: None,
            key_manager,
        }
    }

    pub fn with_pg_pool(mut self, pool: PgPool) -> Self {
        self.pool = Some(pool);
        self
    }

    pub fn with_sessions(mut self) -> Self {
        let store = MemoryStore::new();
        let mut secret = [0u8; 128];
        rand::thread_rng().fill_bytes(&mut secret[..]);
        self.session_layer = Some(
            SessionLayer::new(store, &secret)
                .with_cookie_name("shuttle.sid")
                .with_session_ttl(Some(COOKIE_EXPIRATION))
                .with_secure(true),
        );

        self
    }

    pub fn with_stripe_client(mut self, stripe_client: stripe::Client) -> Self {
        self.stripe_client = Some(stripe_client);
        self
    }

    pub fn with_rds_price_id(mut self, price_id: String) -> Self {
        self.rds_price_id = Some(price_id);
        self
    }

    pub fn into_router(self) -> Router {
        let pool = self.pool.expect("an sqlite pool is required");
        let session_layer = self.session_layer.expect("a session layer is required");
        let stripe_client = self.stripe_client.expect("a stripe client is required");
        let rds_price_id = self.rds_price_id.expect("rds price id is required");

        let user_manager = UserManager {
            pool,
            stripe_client,
        };

        let state = RouterState {
            user_manager: Arc::new(Box::new(user_manager)),
            key_manager: Arc::new(Box::new(self.key_manager)),
            rds_price_id,
        };

        self.router.layer(session_layer).with_state(state)
    }
}

pub async fn serve(router: Router, address: SocketAddr) {
    Server::bind(&address)
        .serve(router.into_make_service())
        .await
        .unwrap_or_else(|_| panic!("Failed to bind to address: {}", address));
}
