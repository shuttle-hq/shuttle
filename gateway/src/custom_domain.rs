use axum::body::boxed;
use std::sync::Arc;
use std::task::{Context, Poll};

use axum::http::Request;
use axum::response::Response;
use futures::future::BoxFuture;
use hyper::Body;
use serde::{Deserialize, Serialize};
use tower::{Layer, Service};

use crate::service::GatewayService;
use crate::Error;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CustomDomain {
    // TODO: update custom domain states, these are just placeholders for now
    Creating,
    Verifying,
    IssuingCertificate,
    Ready,
    Errored,
}

pub struct ChallengeResponder {
    gateway: Arc<GatewayService>,
}

impl ChallengeResponder {
    pub fn new(gateway: Arc<GatewayService>) -> Self {
        Self { gateway }
    }
}

impl<S> Layer<S> for ChallengeResponder {
    type Service = ChallengeResponderMiddleware<S>;

    fn layer(&self, inner: S) -> Self::Service {
        ChallengeResponderMiddleware {
            gateway: self.gateway.clone(),
            inner,
        }
    }
}

pub struct ChallengeResponderMiddleware<S> {
    gateway: Arc<GatewayService>,
    inner: S,
}

impl<S> Service<Request<Body>> for ChallengeResponderMiddleware<S>
where
    S: Service<Request<Body>, Response = Response, Error = Error> + Send + 'static,
    S::Future: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        if !req.uri().path().starts_with("/.well-known/acme-challenge/") {
            let future = self.inner.call(req);
            return Box::pin(async move {
                let response: Response = future.await?;
                Ok(response)
            });
        }

        let token = match req
            .uri()
            .path()
            .strip_prefix("/.well-known/acme-challenge/")
        {
            Some(token) => token.to_string(),
            None => {
                return Box::pin(async {
                    Ok(Response::builder()
                        .status(404)
                        .body(boxed(Body::empty()))
                        .unwrap())
                })
            }
        };

        let gateway = self.gateway.clone();

        Box::pin(async move {
            let (status, body) = match gateway.get_http01_challenge_authorization(&token).await {
                Some(key) => (200, Body::from(key)),
                None => (404, Body::empty()),
            };

            Ok(Response::builder()
                .status(status)
                .body(boxed(body))
                .unwrap())
        })
    }
}
