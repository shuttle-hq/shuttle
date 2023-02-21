use axum::{
    body::{Body, HttpBody},
    headers::{authorization::Bearer, Authorization, Cookie, HeaderMapExt},
    http::Request,
    response::Response,
};
use chrono::{TimeZone, Utc};
use http::{header::CONTENT_TYPE, HeaderValue, StatusCode};
use serde_json::Value;
use shuttle_common::backends::auth::{Claim, ConvertResponse};
use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, RwLock},
    task::{Context, Poll},
    time::Duration,
};
use tower::{Layer, Service};
use tracing::error;
use ttl_cache::TtlCache;

use super::RouterState;

pub trait CacheManagement: Send + Sync {
    fn get(&self, key: &str) -> Option<String>;
    fn insert(&self, key: &str, value: String, ttl: Duration) -> Option<String>;
    fn invalidate(&self, key: &str) -> Option<String>;
}

pub struct CacheManager {
    pub cache: Arc<RwLock<TtlCache<String, String>>>,
}

impl CacheManagement for CacheManager {
    fn get(&self, key: &str) -> Option<String> {
        self.cache.read().unwrap().get(key).cloned()
    }

    fn insert(&self, key: &str, value: String, ttl: Duration) -> Option<String> {
        self.cache
            .write()
            .unwrap()
            .insert(key.to_string(), value, ttl)
    }

    fn invalidate(&self, key: &str) -> Option<String> {
        self.cache.write().unwrap().remove(key)
    }
}

#[derive(Clone)]
pub(crate) struct CacheLayer {
    pub(crate) state: RouterState,
}

impl<S> Layer<S> for CacheLayer {
    type Service = Cache<S>;

    fn layer(&self, inner: S) -> Self::Service {
        Cache {
            inner,
            state: self.state.clone(),
        }
    }
}

#[derive(Clone)]
pub(crate) struct Cache<S> {
    inner: S,
    state: RouterState,
}

impl<S> Service<Request<Body>> for Cache<S>
where
    S: Service<Request<Body>, Response = Response> + Send + 'static,
    S::Future: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future =
        Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send + 'static>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, request: Request<Body>) -> Self::Future {
        let path = request.uri().path();

        // If the endpoint to convert a key or a cookie to a jwt is called, try the cache.
        if ["/auth/key", "/auth/session"].contains(&path) {
            let mut key = None;

            if let Ok(Some(token)) = request.headers().typed_try_get::<Authorization<Bearer>>() {
                key = Some(token.token().trim().to_string());
            };

            if let Ok(Some(cookie)) = request.headers().typed_try_get::<Cookie>() {
                if let Some(id) = cookie.get("shuttle.sid") {
                    key = Some(id.to_string())
                };
            };

            let Some(key) = key else {
                // Cookie and API key are missing, return 401.
                return Box::pin(async move {
                    Ok(Response::builder()
                        .status(StatusCode::UNAUTHORIZED)
                        .body(Default::default())
                        .unwrap())
                });
            };

            if let Some(jwt) = self.state.cache_manager.get(&key) {
                // Token is cached and not expired, return it in the response.
                let body = serde_json::to_string(&ConvertResponse { token: jwt }).unwrap();

                let body =
                    <Body as HttpBody>::map_err(Body::from(body), axum::Error::new).boxed_unsync();

                return Box::pin(async move {
                    Ok(Response::builder()
                        .status(StatusCode::OK)
                        .header(CONTENT_TYPE, HeaderValue::from_static("application/json"))
                        .body(body)
                        .unwrap())
                });
            } else {
                // Cache was not hit, the convert endpoint will be called by the ShuttleAuthLayer,
                // we will then extract the token from the response, decode it to get the expiration
                // time and cache it.
                let future = self.inner.call(request);

                let public_key = self.state.key_manager.public_key();
                let cache_manager = self.state.cache_manager.clone();

                return Box::pin(async move {
                    let response: Response = future.await?;

                    // Return response directly if it failed.
                    if response.status() != StatusCode::OK {
                        return Ok(response);
                    }

                    // We'll re-use the parts in the response if all goes well.
                    let (parts, body) = response.into_parts();

                    let body = match hyper::body::to_bytes(body).await {
                        Ok(body) => body,
                        Err(error) => {
                            error!(
                                error = &error as &dyn std::error::Error,
                                "failed to get response body"
                            );

                            return Ok(Response::builder()
                                .status(StatusCode::INTERNAL_SERVER_ERROR)
                                .body(Default::default())
                                .unwrap());
                        }
                    };

                    let value: Value = match serde_json::from_slice(&body) {
                        Ok(value) => value,
                        Err(error) => {
                            error!(
                                error = &error as &dyn std::error::Error,
                                "response body is malformed"
                            );

                            return Ok(Response::builder()
                                .status(StatusCode::BAD_REQUEST)
                                .body(Default::default())
                                .unwrap());
                        }
                    };

                    let Some(jwt) = value["token"].as_str() else {
                        error!("response json is missing 'token' key");

                        return Ok(Response::builder()
                                .status(StatusCode::UNAUTHORIZED)
                                .body(Default::default())
                                .unwrap());
                    };

                    let public_key = public_key.clone();

                    let claim = match Claim::from_token(jwt, &public_key) {
                        Ok(claim) => claim,
                        Err(status) => {
                            return Ok(Response::builder()
                                .status(status)
                                .body(Default::default())
                                .unwrap());
                        }
                    };

                    // Expiration time (as UTC timestamp).
                    let exp = claim.exp;

                    let Some(expiration_timestamp) = Utc
                        .timestamp_opt(exp as i64, 0)
                        .single()
                        else {
                            error!("expiration timestamp is out of range number");

                            return Ok(Response::builder()
                                .status(StatusCode::INTERNAL_SERVER_ERROR)
                                .body(Default::default())
                                .unwrap());
                        };

                    let duration = expiration_timestamp - Utc::now();

                    // Cache the token.
                    cache_manager.insert(
                        key.as_str(),
                        jwt.to_owned(),
                        Duration::from_secs(duration.num_seconds() as u64),
                    );

                    // Request succeeded and JWT was cached. Convert the body bytes back into a HttpBody,
                    // and return it along with the original response parts.
                    let body =
                        <Body as HttpBody>::map_err(body.into(), axum::Error::new).boxed_unsync();

                    Ok(Response::from_parts(parts, body))
                });
            }
        }

        let future = self.inner.call(request);

        Box::pin(async move {
            let response: Response = future.await?;

            Ok(response)
        })
    }
}
