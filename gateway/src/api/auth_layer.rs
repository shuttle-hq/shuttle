use std::{
    convert::Infallible,
    net::{Ipv4Addr, SocketAddr},
};

use axum::{
    body::{boxed, HttpBody},
    response::Response,
};
use futures::future::BoxFuture;
use http::{Request, StatusCode};
use hyper::{
    client::{connect::dns::GaiResolver, HttpConnector},
    Body, Client,
};
use hyper_reverse_proxy::ReverseProxy;
use once_cell::sync::Lazy;
use opentelemetry::global;
use opentelemetry_http::HeaderInjector;
use tower::{Layer, Service};
use tracing::{error, Span};
use tracing_opentelemetry::OpenTelemetrySpanExt;

static PROXY_CLIENT: Lazy<ReverseProxy<HttpConnector<GaiResolver>>> =
    Lazy::new(|| ReverseProxy::new(Client::new()));

#[derive(Clone)]
pub struct ShuttleAuthLayer {
    auth_address: SocketAddr,
}

impl ShuttleAuthLayer {
    pub fn new(auth_address: SocketAddr) -> Self {
        Self { auth_address }
    }
}

impl<S> Layer<S> for ShuttleAuthLayer {
    type Service = ShuttleAuthService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        ShuttleAuthService {
            inner,
            auth_address: self.auth_address,
        }
    }
}

#[derive(Clone)]
pub struct ShuttleAuthService<S> {
    inner: S,
    auth_address: SocketAddr,
}

impl<S> Service<Request<Body>> for ShuttleAuthService<S>
where
    S: Service<Request<Body>, Response = Response>,
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
        let pass_to_auth = match req.uri().path() {
            "/login" | "/logout" => true,
            other => other.starts_with("/users"),
        };

        if pass_to_auth {
            let target_url = format!("http://{}", self.auth_address);

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
            let future = self.inner.call(req);

            Box::pin(async move {
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
            })
        }
    }
}
