use axum::{
    body::{Body, HttpBody},
    headers::{authorization::Bearer, Authorization, Cookie, HeaderMapExt},
    http::Request,
    response::Response,
};
use http::{header::CONTENT_TYPE, HeaderValue, StatusCode};
use shuttle_common::backends::auth::ConvertResponse;
use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, RwLock},
    task::{Context, Poll},
    time::Duration,
};
use tower::{Layer, Service};
use ttl_cache::TtlCache;

use super::RouterState;

pub trait CacheManagement: Send + Sync {
    fn get(&self, key: &str) -> Option<String>;
    fn insert(&self, key: &str, value: String, ttl: Duration) -> Option<String>;
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

            // Cookie and API key are missing, return 401.
            let Some(key) = key else {
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
            }
        }

        let future = self.inner.call(request);

        Box::pin(async move {
            let response: Response = future.await?;

            Ok(response)
        })
    }
}
