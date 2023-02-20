use axum::{
    body::{Body, HttpBody},
    headers::{authorization::Bearer, Authorization, HeaderMapExt},
    http::Request,
    response::Response,
};
use http::{header::CONTENT_TYPE, HeaderValue, StatusCode};
use shuttle_common::backends::auth::ConvertResponse;
use std::{
    future::Future,
    pin::Pin,
    str::FromStr,
    task::{Context, Poll},
};
use tower::{Layer, Service};

use crate::user::Key;

use super::RouterState;

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
        let Ok(Some(token)) = request.headers().typed_try_get::<Authorization<Bearer>>() else {
            return Box::pin(async move {
                Ok(Response::builder()
                    .status(StatusCode::UNAUTHORIZED)
                    .body(Default::default())
                    .unwrap())
            })
        };

        let key = Key::from_str(token.token().trim()).expect("key to be valid string");

        if let Some(jwt) = self.state.cache.read().unwrap().get(&key) {
            // Token is cached and not expired, return it in the response.
            let body = serde_json::to_string(&ConvertResponse {
                token: jwt.to_owned(),
            })
            .unwrap();

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

        let future = self.inner.call(request);

        Box::pin(async move {
            let response: Response = future.await?;

            Ok(response)
        })
    }
}
