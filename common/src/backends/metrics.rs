use std::marker::PhantomData;
use std::time::Duration;
use std::{collections::HashMap, convert::Infallible};

use async_trait::async_trait;
use axum::body::{Body, BoxBody};
use axum::extract::{FromRequestParts, MatchedPath, Path};
use axum::http::{request::Parts, Request, Response};
use opentelemetry::global;
use opentelemetry_http::HeaderExtractor;
use tower_http::classify::{ServerErrorsAsFailures, SharedClassifier};
use tower_http::trace::DefaultOnRequest;
use tracing::{debug, debug_span, field, Span, Value};
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

type FnFields = fn(&Request<Body>) -> Vec<(&str, Box<dyn Value>)>;

/// Record the default tracing information for each request. These defaults are:
/// - The URI
/// - The method
/// - The status code
/// - The request path
pub struct TraceLayer<MakeSpan = MakeSpanSimple> {
    fn_extra_fields: FnFields,
    make_span_type: PhantomData<MakeSpan>,
}

impl<MakeSpan> Default for TraceLayer<MakeSpan> {
    fn default() -> Self {
        Self::new()
    }
}

impl<MakeSpan> TraceLayer<MakeSpan> {
    pub fn new() -> Self {
        Self {
            fn_extra_fields: |_| Default::default(),
            make_span_type: PhantomData,
        }
    }

    /// Set a function to record extra tracing fields. These might be fields set by [Metrics].
    ///
    /// # Example
    /// ```
    /// TraceLayer::new()
    ///     .extra_fields(|_| vec![("request.params.account_name", &field::Empty)])
    ///     .build()
    /// ```
    pub fn extra_fields(mut self, fn_extra_fields: FnFields) -> Self {
        self.fn_extra_fields = fn_extra_fields;

        self
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
            .make_span_with(MakeSpan::new(self.fn_extra_fields))
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
    /// Switch to the span maker which add propagation details from the request headers
    pub fn with_propagation(self) -> Self {
        self
    }
}

/// Helper trait to make a new span maker
pub trait MakeSpanBuilder {
    fn new(fn_extra_fields: FnFields) -> Self;
}

/// Simple span maker which records the default traces with the extra given by the user
#[derive(Clone)]
pub struct MakeSpanSimple {
    fn_extra_fields: FnFields,
}

impl MakeSpanBuilder for MakeSpanSimple {
    fn new(fn_extra_fields: FnFields) -> Self {
        Self { fn_extra_fields }
    }
}

impl tower_http::trace::MakeSpan<Body> for MakeSpanSimple {
    fn make_span(&mut self, request: &Request<Body>) -> Span {
        get_span(request, self.fn_extra_fields)
    }
}

/// Span maker which records the default traces, those given by the user and extract a propagation context
/// from the request headers.
#[derive(Clone)]
pub struct MakeSpanPropagation {
    fn_extra_fields: FnFields,
}

impl MakeSpanBuilder for MakeSpanPropagation {
    fn new(fn_extra_fields: FnFields) -> Self {
        Self { fn_extra_fields }
    }
}

impl tower_http::trace::MakeSpan<Body> for MakeSpanPropagation {
    fn make_span(&mut self, request: &Request<Body>) -> Span {
        let span = get_span(request, self.fn_extra_fields);

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

#[inline]
fn get_span(request: &Request<Body>, fn_extra_fields: FnFields) -> Span {
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
    );

    let extra_fields = (fn_extra_fields)(request);

    for (key, value) in extra_fields {
        span.record(key, value);
    }

    span
}
