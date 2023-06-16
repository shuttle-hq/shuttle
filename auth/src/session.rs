use std::{
    str::FromStr,
    task::{Context, Poll},
};

use cookie::Cookie;
use hmac::{Hmac, Mac};
use http::header::COOKIE;
use ring::rand::{SecureRandom, SystemRandom};
use sha2::Sha256;
use shuttle_common::claims::{AccountTier, ResponseFuture};
use tonic::body::BoxBody;
use tower::{Layer, Service};
use tracing::error;

use crate::{secrets::KeyManager, user::AccountName, Dal};

pub const COOKIE_NAME: &str = "shuttle.sid";
pub const COOKIE_EXPIRATION: i64 = 60 * 60 * 24; // One day
const BASE64_DIGEST_LEN: usize = 44;

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

// the following is reused verbatim from
// https://github.com/SergioBenitez/cookie-rs/blob/master/src/secure/signed.rs#L33-L43
/// Signs the cookie's value providing integrity and authenticity.
pub fn sign_cookie(key: &cookie::Key, cookie: &mut Cookie<'_>) {
    // Compute HMAC-SHA256 of the cookie's value.
    let mut mac = Hmac::<Sha256>::new_from_slice(key.signing()).expect("good key");
    mac.update(cookie.value().as_bytes());

    // Cookie's new value is [MAC | original-value].
    let mut new_value = base64::encode(mac.finalize().into_bytes());
    new_value.push_str(cookie.value());
    cookie.set_value(new_value);
}

// the following is reused verbatim from
// https://github.com/SergioBenitez/cookie-rs/blob/master/src/secure/signed.rs#L45-L63
/// Given a signed value `str` where the signature is prepended to `value`,
/// verifies the signed value and returns it. If there's a problem, returns
/// an `Err` with a string describing the issue.
pub fn verify_signature(key: &cookie::Key, cookie_value: &str) -> Result<String, &'static str> {
    if cookie_value.len() < BASE64_DIGEST_LEN {
        return Err("length of value is <= BASE64_DIGEST_LEN");
    }

    // Split [MAC | original-value] into its two parts.
    let (digest_str, value) = cookie_value.split_at(BASE64_DIGEST_LEN);
    let digest = base64::decode(digest_str).map_err(|_| "bad base64 digest")?;

    // Perform the verification.
    let mut mac = Hmac::<Sha256>::new_from_slice(key.signing()).expect("good key");

    mac.update(value.as_bytes());
    mac.verify_slice(&digest)
        .map(|_| value.to_string())
        .map_err(|_| "value did not verify")
}

#[derive(Clone)]
pub(crate) struct SessionUser {
    pub account_name: AccountName,
    pub account_tier: AccountTier,
}

/// This state will be available as an extension on requests that have a valid cookie,
/// it can then be accessed in handlers to verify a user is logged in and get user data.
#[derive(Clone)]
pub(crate) struct SessionState<D: Dal + Send + Sync + 'static> {
    cached_user: Option<SessionUser>,
    session_store: D,
    token: SessionToken,
}

impl<D> SessionState<D>
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
pub struct SessionLayer<D, K>
where
    D: Dal + Send + Clone + 'static,
    K: KeyManager + Send + Sync + 'static,
{
    key_manager: K,
    session_store: D,
}

impl<D, K> SessionLayer<D, K>
where
    D: Dal + Send + Clone + 'static,
    K: KeyManager + Send + Sync + 'static,
{
    pub fn new(dal: D, key_manager: K) -> Self {
        Self {
            key_manager,
            session_store: dal,
        }
    }
}

impl<S, D, K> Layer<S> for SessionLayer<D, K>
where
    D: Dal + Send + Clone + 'static,
    K: KeyManager + Send + Sync + Clone + 'static,
{
    type Service = Session<S, D, K>;

    fn layer(&self, service: S) -> Self::Service {
        Session {
            key_manager: self.key_manager.clone(),
            inner: service,
            session_store: self.session_store.clone(),
        }
    }
}

#[derive(Clone)]
pub struct Session<S, D, K>
where
    D: Dal + Send + 'static,
    K: KeyManager + Send + Sync + 'static,
{
    key_manager: K,
    inner: S,
    session_store: D,
}

impl<S, Body, D, K> Service<http::Request<Body>> for Session<S, D, K>
where
    S: Service<http::Request<Body>, Response = http::Response<BoxBody>> + Clone + Send + 'static,
    S::Future: Send + 'static,
    D: Dal + Send + Sync + Clone + 'static,
    K: KeyManager + Send + Sync + Clone + 'static,
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
            .filter(|cookie| cookie.name() == COOKIE_NAME)
            .find_map(|cookie| {
                let clone = self.key_manager.clone();
                let key_manager = std::mem::replace(&mut self.key_manager, clone);

                verify_signature(key_manager.cookie_secret(), cookie.value()).ok()
            })
            .and_then(|cookie_value| cookie_value.parse::<SessionToken>().ok());

        if let Some(token) = session_token {
            let clone = self.session_store.clone();
            let session_store = std::mem::replace(&mut self.session_store, clone);

            req.extensions_mut().insert(SessionState {
                cached_user: None,
                session_store,
                token,
            });
        }

        let future = inner.call(req);

        ResponseFuture(future)
    }
}
