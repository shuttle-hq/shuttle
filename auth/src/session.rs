use std::{
    str::FromStr,
    task::{Context, Poll},
    time::Duration,
};

use http::header::COOKIE;
use ring::rand::{SecureRandom, SystemRandom};
use shuttle_common::claims::ResponseFuture;
use tonic::body::BoxBody;
use tower::{Layer, Service};
use tracing::error;

use crate::{user::AccountName, AccountTier, Dal};

pub const COOKIE_NAME: &str = "shuttle.sid";
pub const COOKIE_EXPIRATION: Duration = Duration::from_secs(60 * 60 * 24); // One day

#[derive(Clone, Copy, Debug)]
pub struct SessionToken(u128);

impl FromStr for SessionToken {
    type Err = <u128 as FromStr>::Err;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.parse().map(Self)
    }
}

impl SessionToken {
    pub fn generate(random: &SystemRandom) -> Self {
        let mut u128_pool = [0u8; 16];

        random
            .fill(&mut u128_pool)
            .expect("random should fill u128");

        Self(u128::from_le_bytes(u128_pool))
    }

    pub fn into_cookie_value(self) -> String {
        self.0.to_string()
    }

    pub fn into_database_value(self) -> Vec<u8> {
        self.0.to_le_bytes().to_vec()
    }
}

#[derive(Clone)]
pub(crate) struct SessionUser {
    pub account_name: AccountName,
    pub account_tier: AccountTier,
}

/// This state will be available as an extension on requests that have a valid cookie,
/// it can then be accessed in handlers to verify a user is logged in and get user data.
#[derive(Clone)]
pub(crate) struct AuthState<D: Dal + Send + Sync + 'static> {
    cached_user: Option<SessionUser>,
    session_store: D,
    token: SessionToken,
}

impl<D> AuthState<D>
where
    D: Dal + Send + Sync + 'static,
{
    pub async fn user(&mut self) -> Option<&SessionUser> {
        let Self {
            cached_user,
            session_store,
            token,
        } = self;

        if cached_user.is_none() {
            let user = session_store
                .get_user_by_session_token(&token.into_database_value())
                .await;

            match user {
                Ok(user) => {
                    *cached_user = Some(SessionUser {
                        account_name: user.name,
                        account_tier: user.account_tier,
                    });
                }
                Err(error) => {
                    error!(error = ?error, "failed to get user by session token");
                }
            }
        }

        cached_user.as_ref()
    }

    pub fn token(&self) -> &SessionToken {
        &self.token
    }
}

#[derive(Clone)]
pub struct SessionLayer<D: Dal + Send + 'static> {
    session_store: D,
}

impl<D> SessionLayer<D>
where
    D: Dal + Send + Clone + 'static,
{
    pub fn new(dal: D) -> Self {
        Self { session_store: dal }
    }
}

impl<S, D> Layer<S> for SessionLayer<D>
where
    D: Dal + Send + Clone + 'static,
{
    type Service = Session<S, D>;

    fn layer(&self, service: S) -> Self::Service {
        Session {
            inner: service,
            session_store: self.session_store.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Session<S, D>
where
    D: Dal + Send + 'static,
{
    inner: S,
    session_store: D,
}

impl<S, Body, D> Service<http::Request<Body>> for Session<S, D>
where
    S: Service<http::Request<Body>, Response = http::Response<BoxBody>> + Clone + Send + 'static,
    S::Future: Send + 'static,
    D: Dal + Send + Sync + Clone + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = ResponseFuture<S::Future>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut req: http::Request<Body>) -> Self::Future {
        // This is necessary because tonic internally uses `tower::buffer::Buffer`.
        // See https://github.com/tower-rs/tower/issues/547#issuecomment-767629149
        // for details on why this is necessary
        let clone = self.inner.clone();
        let mut inner = std::mem::replace(&mut self.inner, clone);

        let clone = self.session_store.clone();
        let session_store = std::mem::replace(&mut self.session_store, clone);

        let session_token = req
            .headers()
            .get_all(COOKIE)
            .iter()
            .filter_map(|cookie| {
                cookie
                    .to_str()
                    .ok()
                    .and_then(|cookie| cookie.parse::<cookie::Cookie>().ok())
            })
            .find_map(|cookie| {
                (cookie.name() == COOKIE_NAME).then(move || cookie.value().to_owned())
            })
            .and_then(|cookie_value| cookie_value.parse::<SessionToken>().ok());

        if let Some(token) = session_token {
            req.extensions_mut().insert(AuthState {
                cached_user: None,
                session_store,
                token,
            });
        }

        let future = inner.call(req);

        ResponseFuture(future)
    }
}
