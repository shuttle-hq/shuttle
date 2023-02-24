use std::{convert::Infallible, net::Ipv4Addr, sync::Arc, time::Duration};

use axum::{
    body::{boxed, HttpBody},
    headers::{authorization::Bearer, Authorization, Cookie, Header, HeaderMapExt},
    response::Response,
};
use chrono::{TimeZone, Utc};
use futures::future::BoxFuture;
use http::{Request, StatusCode, Uri};
use hyper::{
    client::{connect::dns::GaiResolver, HttpConnector},
    Body, Client,
};
use hyper_reverse_proxy::ReverseProxy;
use once_cell::sync::Lazy;
use opentelemetry::global;
use opentelemetry_http::HeaderInjector;
use shuttle_common::backends::auth::ConvertResponse;
use tower::{Layer, Service};
use tracing::{error, trace, Span};
use tracing_opentelemetry::OpenTelemetrySpanExt;

use super::cache::CacheManagement;

static PROXY_CLIENT: Lazy<ReverseProxy<HttpConnector<GaiResolver>>> =
    Lazy::new(|| ReverseProxy::new(Client::new()));

/// The idea of this layer is to do two things:
/// 1. Forward all user related routes (`/login`, `/logout`, `/users/*`, etc) to our auth service
/// 2. Upgrade all Authorization Bearer keys and session cookies to JWT tokens for internal
/// communication inside and below gateway, fetching the JWT token from a ttl-cache if it isn't expired,
/// and inserting it in the cache if it isn't there.
#[derive(Clone)]
pub struct ShuttleAuthLayer {
    auth_uri: Uri,
    cache_manager: Arc<Box<dyn CacheManagement>>,
}

impl ShuttleAuthLayer {
    pub fn new(auth_uri: Uri, cache_manager: Arc<Box<dyn CacheManagement>>) -> Self {
        Self {
            auth_uri,
            cache_manager,
        }
    }
}

impl<S> Layer<S> for ShuttleAuthLayer {
    type Service = ShuttleAuthService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        ShuttleAuthService {
            inner,
            auth_uri: self.auth_uri.clone(),
            cache_manager: self.cache_manager.clone(),
        }
    }
}

#[derive(Clone)]
pub struct ShuttleAuthService<S> {
    inner: S,
    auth_uri: Uri,
    cache_manager: Arc<Box<dyn CacheManagement>>,
}

impl<S> Service<Request<Body>> for ShuttleAuthService<S>
where
    S: Service<Request<Body>, Response = Response> + Send + Clone + 'static,
    S::Future: Send + 'static,
{
    type Response = S::Response;
    type Error = Infallible;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        match self.inner.poll_ready(cx) {
            std::task::Poll::Ready(_) => std::task::Poll::Ready(Ok(())),
            std::task::Poll::Pending => std::task::Poll::Pending,
        }
    }

    fn call(&mut self, mut req: Request<Body>) -> Self::Future {
        // Pass through status page
        if req.uri().path() == "/" {
            let future = self.inner.call(req);

            return Box::pin(async move {
                match future.await {
                    Ok(response) => Ok(response),
                    Err(_) => {
                        error!("unexpected internal error from gateway");

                        Ok(Response::builder()
                            .status(StatusCode::SERVICE_UNAVAILABLE)
                            .body(boxed(Body::empty()))
                            .unwrap())
                    }
                }
            });
        }

        let forward_to_auth = match req.uri().path() {
            "/login" | "/logout" => true,
            other => other.starts_with("/users"),
        };

        // If logout is called, invalidate the cached JWT for the callers cookie.
        if req.uri().path() == "/logout" {
            let cache_manager = self.cache_manager.clone();

            if let Ok(Some(cookie)) = req.headers().typed_try_get::<Cookie>() {
                if let Some(key) = cookie.get("shuttle.sid").map(|id| id.to_string()) {
                    cache_manager.invalidate(&key);
                }
            };
        }

        if forward_to_auth {
            let target_url = self.auth_uri.to_string();

            let cx = Span::current().context();

            global::get_text_map_propagator(|propagator| {
                propagator.inject_context(&cx, &mut HeaderInjector(req.headers_mut()))
            });

            Box::pin(async move {
                let response = PROXY_CLIENT
                    .call(Ipv4Addr::LOCALHOST.into(), &target_url, req)
                    .await;

                match response {
                    Ok(res) => {
                        let (parts, body) = res.into_parts();
                        let body =
                            <Body as HttpBody>::map_err(body, axum::Error::new).boxed_unsync();

                        Ok(Response::from_parts(parts, body))
                    }
                    Err(error) => {
                        error!(?error, "failed to call authentication service");

                        Ok(Response::builder()
                            .status(StatusCode::SERVICE_UNAVAILABLE)
                            .body(boxed(Body::empty()))
                            .unwrap())
                    }
                }
            })
        } else {
            // Enrich the current key | session

            // TODO: read this page to get rid of this clone
            // https://github.com/tower-rs/tower/blob/master/guides/building-a-middleware-from-scratch.md
            let mut this = self.clone();

            Box::pin(async move {
                let mut auth_details = None;
                let mut cache_key = None;

                if let Some(bearer) = req.headers().typed_get::<Authorization<Bearer>>() {
                    cache_key = Some(bearer.token().trim().to_string());
                    auth_details = Some(make_token_request("/auth/key", bearer));
                }

                if let Some(cookie) = req.headers().typed_get::<Cookie>() {
                    if let Some(id) = cookie.get("shuttle.sid") {
                        cache_key = Some(id.to_string());
                        auth_details = Some(make_token_request("/auth/session", cookie));
                    };
                }

                // Only if there is something to upgrade
                if let Some(token_request) = auth_details {
                    let target_url = this.auth_uri.to_string();

                    if let Some(key) = cache_key {
                        // Check if the token is cached.
                        if let Some(token) = this.cache_manager.get(&key) {
                            trace!("JWT cache hit, setting token from cache on request");

                            // Token is cached and not expired, return it in the response.
                            req.headers_mut()
                                .typed_insert(Authorization::bearer(&token).unwrap());
                        } else {
                            trace!("JWT cache missed, sending convert token request");

                            // Token is not in the cache, send a convert request.
                            let token_response = match PROXY_CLIENT
                                .call(Ipv4Addr::LOCALHOST.into(), &target_url, token_request)
                                .await
                            {
                                Ok(res) => res,
                                Err(error) => {
                                    error!(?error, "failed to call authentication service");

                                    return Ok(Response::builder()
                                        .status(StatusCode::SERVICE_UNAVAILABLE)
                                        .body(boxed(Body::empty()))
                                        .unwrap());
                                }
                            };

                            // Bubble up auth errors
                            if token_response.status() != StatusCode::OK {
                                let (parts, body) = token_response.into_parts();
                                let body = <Body as HttpBody>::map_err(body, axum::Error::new)
                                    .boxed_unsync();

                                return Ok(Response::from_parts(parts, body));
                            }

                            let body = match hyper::body::to_bytes(token_response.into_body()).await
                            {
                                Ok(body) => body,
                                Err(error) => {
                                    error!(
                                        error = &error as &dyn std::error::Error,
                                        "failed to get response body"
                                    );

                                    return Ok(Response::builder()
                                        .status(StatusCode::INTERNAL_SERVER_ERROR)
                                        .body(boxed(Body::empty()))
                                        .unwrap());
                                }
                            };

                            let response: ConvertResponse = match serde_json::from_slice(&body) {
                                Ok(response) => response,
                                Err(error) => {
                                    error!(
                                        error = &error as &dyn std::error::Error,
                                        "failed to convert body to ConvertResponse"
                                    );

                                    return Ok(Response::builder()
                                        .status(StatusCode::INTERNAL_SERVER_ERROR)
                                        .body(boxed(Body::empty()))
                                        .unwrap());
                                }
                            };

                            match extract_token_expiration(response.token.clone()) {
                                Ok(expiration) => {
                                    // Cache the token.
                                    this.cache_manager.insert(
                                        key.as_str(),
                                        response.token.clone(),
                                        expiration,
                                    );
                                }
                                Err(status) => {
                                    error!(
                                        "failed to extract token expiration before inserting into cache"
                                    );
                                    return Ok(Response::builder()
                                        .status(status)
                                        .body(boxed(Body::empty()))
                                        .unwrap());
                                }
                            };

                            trace!("token inserted in cache, request proceeding");
                            req.headers_mut()
                                .typed_insert(Authorization::bearer(&response.token).unwrap());
                        }
                    };
                }

                match this.inner.call(req).await {
                    Ok(response) => Ok(response),
                    Err(_) => {
                        error!("unexpected internal error from gateway");

                        Ok(Response::builder()
                            .status(StatusCode::SERVICE_UNAVAILABLE)
                            .body(boxed(Body::empty()))
                            .unwrap())
                    }
                }
            })
        }
    }
}

fn extract_token_expiration(token: String) -> Result<Duration, StatusCode> {
    let (_header, rest) = token
        .split_once('.')
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;

    let (claim, _sig) = rest
        .split_once('.')
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;

    let claim = base64::decode_config(claim, base64::URL_SAFE_NO_PAD)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let claim: serde_json::Map<String, serde_json::Value> =
        serde_json::from_slice(&claim).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let exp = claim["exp"]
        .as_i64()
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;

    let expiration_timestamp = Utc
        .timestamp_opt(exp, 0)
        .single()
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;

    let duration = expiration_timestamp - Utc::now();

    Ok(std::time::Duration::from_secs(duration.num_seconds() as u64))
}

fn make_token_request(uri: &str, header: impl Header) -> Request<Body> {
    let mut token_request = Request::builder().uri(uri);
    token_request
        .headers_mut()
        .expect("manual request to be valid")
        .typed_insert(header);

    let cx = Span::current().context();

    global::get_text_map_propagator(|propagator| {
        propagator.inject_context(
            &cx,
            &mut HeaderInjector(token_request.headers_mut().expect("request to be valid")),
        )
    });

    token_request
        .body(Body::empty())
        .expect("manual request to be valid")
}
