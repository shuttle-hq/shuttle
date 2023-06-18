use std::{convert::Infallible, fmt::Debug, sync::Arc, time::Duration};

use axum::{
    body::boxed,
    headers::{authorization::Bearer, Authorization, HeaderMapExt},
    response::Response,
};
use axum_extra::extract::CookieJar;
use futures::{
    future::{BoxFuture, Either},
    Future,
};
use http::{header::COOKIE, HeaderMap, Request, StatusCode};
use hyper::Body;
use shuttle_common::{
    backends::{auth::COOKIE_NAME, cache::CacheManagement},
    claims::InjectPropagation,
};
use shuttle_proto::auth::{
    auth_client::AuthClient, ApiKeyRequest, ConvertCookieRequest, TokenResponse,
};
use tonic::{metadata::MetadataValue, transport::Channel};
use tonic::{Request as TonicRequest, Status};
use tower::{Layer, Service};
use tracing::{error, trace};

/// Time to cache tokens for. Currently tokens take 15 minutes to expire (see [EXP_MINUTES]) which leaves a 10 minutes
/// buffer (EXP_MINUTES - CACHE_MINUTES). We want the buffer to be atleast as long as the longest builds which has
/// been observed to be around 5 minutes.
const CACHE_MINUTES: u64 = 5;

/// The idea of this layer is to do two things:
/// 1. Forward all user related routes (`/login`, `/logout`, `/users/*`, etc) to our auth service
/// 2. Upgrade all Authorization Bearer keys and session cookies to JWT tokens for internal
/// communication inside and below gateway, fetching the JWT token from a ttl-cache if it isn't expired,
/// and inserting it in the cache if it isn't there.
#[derive(Clone)]
pub struct ShuttleAuthLayer {
    cache_manager: Arc<Box<dyn CacheManagement<Value = String>>>,
    auth_client: AuthClient<InjectPropagation<Channel>>,
}

impl ShuttleAuthLayer {
    pub fn new(
        cache_manager: Arc<Box<dyn CacheManagement<Value = String>>>,
        auth_client: AuthClient<InjectPropagation<Channel>>,
    ) -> Self {
        Self {
            cache_manager,
            auth_client,
        }
    }
}

impl<S> Layer<S> for ShuttleAuthLayer {
    type Service = ShuttleAuthService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        ShuttleAuthService {
            inner,
            cache_manager: self.cache_manager.clone(),
            auth_client: self.auth_client.clone(),
        }
    }
}

#[derive(Clone)]
pub struct ShuttleAuthService<S> {
    inner: S,
    cache_manager: Arc<Box<dyn CacheManagement<Value = String>>>,
    auth_client: AuthClient<InjectPropagation<Channel>>,
}

impl<S> Service<Request<Body>> for ShuttleAuthService<S>
where
    S: Service<Request<Body>, Response = Response> + Send + Clone + 'static,
    S::Error: Debug,
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

        // TODO: read this page to get rid of this clone
        // https://github.com/tower-rs/tower/blob/master/guides/building-a-middleware-from-scratch.md
        let mut this = self.clone();

        // Enrich the current key | session
        Box::pin(async move {
            // Only if there is something to upgrade
            if let Some((cache_key, token_request)) =
                cache_key_and_token_req(req.headers(), &mut this.auth_client)
            {
                // Check if the token is cached.
                if let Some(token) = this.cache_manager.get(&cache_key) {
                    trace!("JWT cache hit, setting token from cache on request");

                    // Token is cached and not expired, return it in the response.
                    req.headers_mut()
                        .typed_insert(Authorization::bearer(&token).unwrap());
                } else {
                    trace!("JWT cache missed, sending convert token request");

                    // Token is not in the cache, send a convert request with either a cookie
                    // or an api-key bearer token from the request headers.
                    let token_response = match token_request.await {
                        Ok(res) => res,
                        Err(error) => {
                            error!(?error, "failed to call authentication service");

                            return Ok(Response::builder()
                                .status(StatusCode::SERVICE_UNAVAILABLE)
                                .body(boxed(Body::empty()))
                                .unwrap());
                        }
                    }
                    .into_inner();

                    let bearer =
                        Authorization::bearer(&token_response.token).expect("bearer token");

                    this.cache_manager.insert(
                        cache_key.as_str(),
                        token_response.token,
                        Duration::from_secs(CACHE_MINUTES * 60),
                    );

                    trace!("token inserted in cache, request proceeding");
                    req.headers_mut().typed_insert(bearer);
                };
            }

            match this.inner.call(req).await {
                Ok(response) => Ok(response),
                Err(error) => {
                    error!(?error, "unexpected internal error from gateway");

                    Ok(Response::builder()
                        .status(StatusCode::SERVICE_UNAVAILABLE)
                        .body(boxed(Body::empty()))
                        .unwrap())
                }
            }
        })
    }
}

/// Return a [ConvertCookieRequest] or a [ApiKeyRequest] depending on the request headers,
/// and return a future that we can .await if the cache is missed.
fn cache_key_and_token_req<'a>(
    headers: &HeaderMap,
    auth_client: &'a mut AuthClient<InjectPropagation<Channel>>,
) -> Option<(
    String,
    Either<
        impl Future<Output = Result<tonic::Response<TokenResponse>, Status>> + 'a,
        impl Future<Output = Result<tonic::Response<TokenResponse>, Status>> + 'a,
    >,
)> {
    let Some((cache_key, request)) = convert_cookie_request(headers).or_else(|| convert_api_key_request(headers)) else {
        // The headers contain neither a bearer token nor a cookie.
        return None;
    };

    // While the futures resolve to the same output, they are anonymous types, so we
    // use Either to combine them into a single type.
    let future = match request {
        ConvertRequestType::Cookie(cookie_request) => {
            Either::Left(auth_client.convert_cookie(cookie_request))
        }
        ConvertRequestType::Bearer(bearer_request) => {
            Either::Right(auth_client.convert_api_key(bearer_request))
        }
    };

    Some((cache_key, future))
}

enum ConvertRequestType {
    Cookie(TonicRequest<ConvertCookieRequest>),
    Bearer(TonicRequest<ApiKeyRequest>),
}

fn convert_cookie_request(headers: &HeaderMap) -> Option<(String, ConvertRequestType)> {
    let jar = CookieJar::from_headers(headers);

    let Some(cookie) = jar.get(COOKIE_NAME) else {
        return None;
    };

    let Ok(metadata_value) = MetadataValue::try_from(&cookie.to_string()) else {
        return None;
    };

    let mut request = TonicRequest::new(ConvertCookieRequest::default());

    let cache_key = cookie.value().to_string();

    request
        .metadata_mut()
        .insert(COOKIE.as_str(), metadata_value);

    Some((cache_key, ConvertRequestType::Cookie(request)))
}

fn convert_api_key_request(headers: &HeaderMap) -> Option<(String, ConvertRequestType)> {
    let Some(bearer) = headers
        .typed_get::<Authorization<Bearer>>()
        .map(|bearer| bearer.token().trim().to_string()) else {
            return None;
        };

    let request = TonicRequest::new(ApiKeyRequest {
        api_key: bearer.clone(),
    });

    Some((bearer, ConvertRequestType::Bearer(request)))
}
