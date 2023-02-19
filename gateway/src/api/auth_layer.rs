use std::{
    convert::Infallible,
    net::{Ipv4Addr, SocketAddr},
};

use axum::{
    body::{boxed, HttpBody},
    headers::{authorization::Bearer, Authorization, Cookie, Header, HeaderMapExt},
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
use shuttle_common::backends::auth::ConvertResponse;
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
            let mut this = self.clone();

            Box::pin(async move {
                let mut auth_details = None;

                if let Some(bearer) = req.headers().typed_get::<Authorization<Bearer>>() {
                    auth_details = Some(make_token_request("/auth/key", bearer));
                }

                if let Some(cookie) = req.headers().typed_get::<Cookie>() {
                    auth_details = Some(make_token_request("/auth/session", cookie));
                }

                if let Some(token_request) = auth_details {
                    let target_url = format!("http://{}", this.auth_address);

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

                    let body = match hyper::body::to_bytes(token_response.into_body()).await {
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

                    req.headers_mut()
                        .typed_insert(Authorization::bearer(&response.token).unwrap());
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
