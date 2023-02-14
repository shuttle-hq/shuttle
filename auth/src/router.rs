use std::time::Duration;

use axum::{
    body::{Body, BoxBody},
    extract::MatchedPath,
    middleware::from_extractor,
    response::Response,
    routing::{get, post},
    Router,
};
use opentelemetry::global;
use opentelemetry_http::{HeaderExtractor, Request};
use shuttle_common::backends::metrics::Metrics;
use tower_http::trace::TraceLayer;
use tracing::{debug, debug_span, field, Span};
use tracing_opentelemetry::OpenTelemetrySpanExt;

pub fn new() -> Router {
    Router::new()
        .route("/login", post(login))
        .route("/logout", post(logout))
        .route("/auth/session", post(convert_cookie))
        .route("/auth/key", post(convert_key))
        .route("/auth/refresh", post(refresh_token))
        .route("/public-key", get(get_public_key))
        .route("/user/:account_name", get(get_user).post(post_user))
        .route_layer(from_extractor::<Metrics>())
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(|request: &Request<Body>| {
                    let path = if let Some(path) = request.extensions().get::<MatchedPath>() {
                        path.as_str()
                    } else {
                        ""
                    };

                    let span = debug_span!(
                        "request",
                        http.uri = %request.uri(),
                        http.method = %request.method(),
                        http.status_code = field::Empty,
                        // A bunch of extra things for metrics
                        // Should be able to make this clearer once `Valuable` support lands in tracing
                        request.path = path,
                        request.params.account_name = field::Empty,
                    );

                    let parent_context = global::get_text_map_propagator(|propagator| {
                        propagator.extract(&HeaderExtractor(request.headers()))
                    });
                    span.set_parent(parent_context);

                    span
                })
                .on_response(
                    |response: &Response<BoxBody>, latency: Duration, span: &Span| {
                        span.record("http.status_code", response.status().as_u16());
                        debug!(
                            latency = format_args!("{} ns", latency.as_nanos()),
                            "finished processing request"
                        );
                    },
                ),
        )
}

async fn login() {}

async fn logout() {}

async fn convert_cookie() {}

async fn convert_key() {}

async fn refresh_token() {}

async fn get_public_key() {}

async fn get_user() {}

async fn post_user() {}
