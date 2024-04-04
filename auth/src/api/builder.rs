use std::{net::SocketAddr, sync::Arc};

use axum::{
    extract::FromRef,
    middleware::from_extractor,
    routing::{delete, get, post, put},
    Router, Server,
};
use shuttle_backends::{
    client::PermissionsDal,
    metrics::{Metrics, TraceLayer},
    request_span,
};
use sqlx::PgPool;
use tracing::field;

use crate::{
    secrets::{EdDsaManager, KeyManager},
    user::{UserManagement, UserManager},
};

use super::handlers::{
    convert_key, delete_subscription, get_public_key, get_user, get_user_by_name,
    post_subscription, post_user, put_user_reset_key,
};

pub type UserManagerState = Arc<Box<dyn UserManagement>>;
pub type KeyManagerState = Arc<Box<dyn KeyManager>>;

#[derive(Clone)]
pub struct RouterState {
    pub user_manager: UserManagerState,
    pub key_manager: KeyManagerState,
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

pub struct ApiBuilder<P: PermissionsDal> {
    router: Router<RouterState>,
    pool: Option<PgPool>,
    stripe_client: Option<stripe::Client>,
    permissions_client: Option<P>,
    jwt_signing_private_key: Option<String>,
}

impl<P> Default for ApiBuilder<P>
where
    P: PermissionsDal + Send + Sync + 'static,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<P> ApiBuilder<P>
where
    P: PermissionsDal + Send + Sync + 'static,
{
    pub fn new() -> Self {
        let router = Router::new()
            // health check: 200 OK
            .route("/", get(|| async move {}))
            .route("/auth/key", get(convert_key))
            .route("/public-key", get(get_public_key))
            // used by console to get user based on auth0 name
            .route("/users/name/:account_name", get(get_user_by_name))
            // users are created based on auth0 name by console
            .route("/users/:account_name/:account_tier", post(post_user))
            .route("/users/:user_id", get(get_user))
            .route("/users/reset-api-key", put(put_user_reset_key))
            .route("/users/:user_id/subscribe", post(post_subscription))
            .route(
                "/users/:user_id/subscribe/:subscription_id",
                delete(delete_subscription),
            )
            .route_layer(from_extractor::<Metrics>())
            .layer(
                TraceLayer::new(|request| {
                    request_span!(
                        request,
                        request.params.user_id = field::Empty,
                        request.params.account_tier = field::Empty,
                    )
                })
                .with_propagation()
                .build(),
            );

        Self {
            router,
            pool: None,
            stripe_client: None,
            permissions_client: None,
            jwt_signing_private_key: None,
        }
    }

    pub fn with_pg_pool(mut self, pool: PgPool) -> Self {
        self.pool = Some(pool);
        self
    }

    pub fn with_stripe_client(mut self, stripe_client: stripe::Client) -> Self {
        self.stripe_client = Some(stripe_client);
        self
    }

    pub fn with_permissions_client(mut self, permissions_client: P) -> Self {
        self.permissions_client = Some(permissions_client);
        self
    }

    pub fn with_jwt_signing_private_key(mut self, private_key: String) -> Self {
        self.jwt_signing_private_key = Some(private_key);
        self
    }

    pub fn into_router(self) -> Router {
        let pool = self.pool.expect("an sqlite pool is required");
        let stripe_client = self.stripe_client.expect("a stripe client is required");
        let permit_client = self
            .permissions_client
            .expect("a permit client is required");
        let jwt_signing_private_key = self
            .jwt_signing_private_key
            .expect("a jwt signing private key");
        let user_manager = UserManager {
            pool,
            stripe_client,
            permissions_client: permit_client,
        };
        let key_manager = EdDsaManager::new(jwt_signing_private_key);

        let state = RouterState {
            user_manager: Arc::new(Box::new(user_manager)),
            key_manager: Arc::new(Box::new(key_manager)),
        };

        self.router.with_state(state)
    }
}

pub async fn serve(router: Router, address: SocketAddr) {
    Server::bind(&address)
        .serve(router.into_make_service())
        .await
        .unwrap_or_else(|_| panic!("Failed to bind to address: {}", address));
}
