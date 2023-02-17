use std::marker::PhantomData;
use std::time::Duration;
use std::{collections::HashMap, convert::Infallible};

use async_trait::async_trait;
use axum::body::{Body, BoxBody};
use axum::extract::{FromRequestParts, Path};
use axum::http::{request::Parts, Request, Response};
use opentelemetry::global;
use opentelemetry_http::HeaderExtractor;
use tower_http::classify::{ServerErrorsAsFailures, SharedClassifier};
use tower_http::trace::DefaultOnRequest;
use tracing::{debug, Span};
use tracing_opentelemetry::OpenTelemetrySpanExt;

/// Used to record a bunch of metrics info
/// The tracing layer on the server should record a `request.params.<param>` field for each parameter
/// that should be recorded. And the [TraceLayer] can be used to record the default `request.params.<param>`
pub struct Metrics;

#[async_trait]
impl<S> FromRequestParts<S> for Metrics
where
    S: Send + Sync,
{
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        // Get path parameters if they exist
        let Path(path): Path<HashMap<String, String>> =
            match Path::from_request_parts(parts, state).await {
                Ok(path) => path,
                Err(_) => return Ok(Metrics),
            };

        let span = Span::current();

        for (param, value) in path {
            span.record(format!("request.params.{param}").as_str(), value);
        }
        Ok(Metrics)
    }
}

type FnSpan = fn(&Request<Body>) -> Span;

/// Record the tracing information for each request as given by the function to create a span
pub struct TraceLayer<MakeSpan = MakeSpanSimple> {
    fn_span: FnSpan,
    make_span_type: PhantomData<MakeSpan>,
}
impl<MakeSpan> TraceLayer<MakeSpan> {
    /// Create a trace layer using the give function to create spans. The span fields might be set by [Metrics] later.
    ///
    /// # Example
    /// ```
    /// TraceLayer::new(|request| {
    ///     request_span!(
    ///         request,
    ///         request.params.param = field::Empty
    ///     )
    /// })
    ///     .without_propagation()
    ///     .build();
    /// ```
    pub fn new(fn_span: FnSpan) -> Self {
        Self {
            fn_span,
            make_span_type: PhantomData,
        }
    }
}

impl<MakeSpan: tower_http::trace::MakeSpan<Body> + MakeSpanBuilder> TraceLayer<MakeSpan> {
    /// Build the configured tracing layer
    pub fn build(
        self,
    ) -> tower_http::trace::TraceLayer<
        SharedClassifier<ServerErrorsAsFailures>,
        MakeSpan,
        DefaultOnRequest,
        OnResponseStatusCode,
    > {
        tower_http::trace::TraceLayer::new_for_http()
            .make_span_with(MakeSpan::new(self.fn_span))
            .on_response(OnResponseStatusCode)
    }
}

impl TraceLayer<MakeSpanSimple> {
    /// Switch to the span maker which does not add propagation details from the request headers
    pub fn without_propagation(self) -> Self {
        self
    }
}

impl TraceLayer<MakeSpanPropagation> {
    /// Switch to the span maker which adds propagation details from the request headers
    pub fn with_propagation(self) -> Self {
        self
    }
}

/// Helper trait to make a new span maker
pub trait MakeSpanBuilder {
    fn new(fn_span: FnSpan) -> Self;
}

/// Simple span maker which records the span given by the user
#[derive(Clone)]
pub struct MakeSpanSimple {
    fn_span: FnSpan,
}

impl MakeSpanBuilder for MakeSpanSimple {
    fn new(fn_span: FnSpan) -> Self {
        Self { fn_span }
    }
}

impl tower_http::trace::MakeSpan<Body> for MakeSpanSimple {
    fn make_span(&mut self, request: &Request<Body>) -> Span {
        (self.fn_span)(request)
    }
}

/// Span maker which records the span given by the user and extracts a propagation context
/// from the request headers.
#[derive(Clone)]
pub struct MakeSpanPropagation {
    fn_span: FnSpan,
}

impl MakeSpanBuilder for MakeSpanPropagation {
    fn new(fn_span: FnSpan) -> Self {
        Self { fn_span }
    }
}

impl tower_http::trace::MakeSpan<Body> for MakeSpanPropagation {
    fn make_span(&mut self, request: &Request<Body>) -> Span {
        let span = (self.fn_span)(request);

        let parent_context = global::get_text_map_propagator(|propagator| {
            propagator.extract(&HeaderExtractor(request.headers()))
        });
        span.set_parent(parent_context);

        span
    }
}

/// Extract and records the status code from the response. And logs out timing info
#[derive(Clone)]
pub struct OnResponseStatusCode;

impl tower_http::trace::OnResponse<BoxBody> for OnResponseStatusCode {
    fn on_response(self, response: &Response<BoxBody>, latency: Duration, span: &Span) {
        span.record("http.status_code", response.status().as_u16());
        debug!(
            latency = format_args!("{} ns", latency.as_nanos()),
            "finished processing request"
        );
    }
}

/// Simple macro to record the following defaults for each request:
/// - The URI
/// - The method
/// - The status code
/// - The request path
#[macro_export]
macro_rules! request_span {
    ($request:expr, $($field:tt)*) => {
        {
        let path = if let Some(path) = $request.extensions().get::<axum::extract::MatchedPath>() {
            path.as_str()
        } else {
            ""
        };

        tracing::debug_span!(
            "request",
            http.uri = %$request.uri(),
            http.method = %$request.method(),
            http.status_code = tracing::field::Empty,
            // A bunch of extra things for metrics
            // Should be able to make this clearer once `Valuable` support lands in tracing
            request.path = path,
            $($field)*
        )
        }
    };
    ($request:expr) => {
        $crate::request_span!($request, )
    }
}

#[cfg(test)]
mod tests {
    use axum::{
        body::Body, extract::Path, http::Request, http::StatusCode, middleware::from_extractor,
        response::IntoResponse, routing::get, Router,
    };
    use hyper::body;
    use tower::ServiceExt;
    use tracing::field;
    use tracing_fluent_assertions::{AssertionRegistry, AssertionsLayer};
    use tracing_subscriber::{layer::SubscriberExt, Registry};

    use super::{Metrics, TraceLayer};

    async fn hello() -> impl IntoResponse {
        "hello"
    }

    async fn hello_user(Path(user_name): Path<String>) -> impl IntoResponse {
        format!("hello {user_name}")
    }

    #[tokio::test]
    async fn trace_layer() {
        let assertion_registry = AssertionRegistry::default();
        let base_subscriber = Registry::default();
        let subscriber = base_subscriber.with(AssertionsLayer::new(&assertion_registry));
        tracing::subscriber::set_global_default(subscriber).unwrap();

        // Put in own block to make sure assertion to not interfere with the next test
        {
            let router: Router<()> = Router::new()
                .route("/hello", get(hello))
                .route_layer(from_extractor::<Metrics>())
                .layer(
                    TraceLayer::new(|request| request_span!(request))
                        .without_propagation()
                        .build(),
                );

            let request_span = assertion_registry
                .build()
                .with_name("request")
                .with_span_field("http.uri")
                .with_span_field("http.method")
                .with_span_field("http.status_code")
                .with_span_field("request.path")
                .was_closed()
                .finalize();

            let response = router
                .oneshot(
                    Request::builder()
                        .uri("/hello")
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();

            assert_eq!(response.status(), StatusCode::OK);

            let body = body::to_bytes(response.into_body()).await.unwrap();

            assert_eq!(&body[..], b"hello");
            request_span.assert();
        }

        {
            let router: Router<()> = Router::new()
                .route("/hello/:user_name", get(hello_user))
                .route_layer(from_extractor::<Metrics>())
                .layer(
                    TraceLayer::new(|request| {
                        request_span!(
                            request,
                            request.params.user_name = field::Empty,
                            extra = "value"
                        )
                    })
                    .without_propagation()
                    .build(),
                );

            let request_span = assertion_registry
                .build()
                .with_name("request")
                .with_span_field("http.uri")
                .with_span_field("http.method")
                .with_span_field("http.status_code")
                .with_span_field("request.path")
                .with_span_field("request.params.user_name")
                .with_span_field("extra")
                .was_closed()
                .finalize();

            let response = router
                .oneshot(
                    Request::builder()
                        .uri("/hello/ferries")
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();

            assert_eq!(response.status(), StatusCode::OK);

            let body = body::to_bytes(response.into_body()).await.unwrap();

            assert_eq!(&body[..], b"hello ferries");
            request_span.assert();
        }
    }
}
