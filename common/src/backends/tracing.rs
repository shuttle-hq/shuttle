use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

use http::{Request, Response};
use opentelemetry::{
    global,
    runtime::Tokio,
    sdk::{propagation::TraceContextPropagator, trace, Resource},
    KeyValue,
};
use opentelemetry_http::HeaderExtractor;
use opentelemetry_otlp::WithExportConfig;
use pin_project::pin_project;
use tower::{Layer, Service};
use tracing::{
    debug_span, field::Visit, instrument::Instrumented, span, Instrument, Metadata, Span,
    Subscriber,
};
use tracing_opentelemetry::OpenTelemetrySpanExt;
use tracing_subscriber::{fmt, prelude::*, registry::LookupSpan, EnvFilter};

use crate::tracing::JsonVisitor;

pub fn setup_tracing<S>(subscriber: S, service_name: &str)
where
    S: Subscriber + for<'a> LookupSpan<'a> + Send + Sync,
{
    global::set_text_map_propagator(TraceContextPropagator::new());

    let filter_layer = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new("info"))
        .unwrap();
    let fmt_layer = fmt::layer();

    let tracer = opentelemetry_otlp::new_pipeline()
        .tracing()
        .with_exporter(
            opentelemetry_otlp::new_exporter()
                .tonic()
                .with_endpoint("http://otel-collector:4317"),
        )
        .with_trace_config(
            trace::config().with_resource(Resource::new(vec![KeyValue::new(
                "service.name",
                service_name.to_string(),
            )])),
        )
        .install_batch(Tokio)
        .unwrap();
    let otel_layer = tracing_opentelemetry::layer().with_tracer(tracer);

    subscriber
        .with(filter_layer)
        .with(fmt_layer)
        .with(otel_layer)
        .init();
}

/// Layer to extract tracing from headers and set the context on the current span
#[derive(Clone)]
pub struct ExtractPropagationLayer;

impl<S> Layer<S> for ExtractPropagationLayer {
    type Service = ExtractPropagation<S>;

    fn layer(&self, inner: S) -> Self::Service {
        ExtractPropagation { inner }
    }
}

/// Middleware for extracting tracing propagation info and setting them on the currently active span
#[derive(Clone)]
pub struct ExtractPropagation<S> {
    inner: S,
}

#[pin_project]
pub struct ExtractPropagationFuture<F> {
    #[pin]
    response_future: F,
}

impl<F, Body, Error> Future for ExtractPropagationFuture<F>
where
    F: Future<Output = Result<Response<Body>, Error>>,
{
    type Output = Result<Response<Body>, Error>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();

        match this.response_future.poll(cx) {
            Poll::Ready(result) => match result {
                Ok(response) => {
                    Span::current().record("http.status_code", response.status().as_u16());

                    Poll::Ready(Ok(response))
                }
                other => Poll::Ready(other),
            },

            Poll::Pending => Poll::Pending,
        }
    }
}

impl<S, Body, ResponseBody> Service<Request<Body>> for ExtractPropagation<S>
where
    S: Service<Request<Body>, Response = Response<ResponseBody>> + Send + 'static,
    S::Future: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = ExtractPropagationFuture<Instrumented<S::Future>>;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        let span = debug_span!(
            "request",
            http.uri = %req.uri(),
            http.method = %req.method(),
            http.status_code = tracing::field::Empty,
        );

        let parent_context = global::get_text_map_propagator(|propagator| {
            propagator.extract(&HeaderExtractor(req.headers()))
        });

        span.set_parent(parent_context);

        let response_future = self.inner.call(req).instrument(span);

        ExtractPropagationFuture { response_future }
    }
}

/// Record a log for a deployment task
pub trait DeploymentLogRecorder {
    fn record_log(&self, deployment_id: &str, visitor: JsonVisitor, metadata: &Metadata);
}

/// Tracing layer to capture logs that relate to a deployment task.
///
/// This causes any functions instrumented with the `deployment_id` attribute to have its logs accosiated with the
/// deployment. Thus, the instrument span acts as the context for logs to capture.
///
/// # Example
/// ```
/// use shuttle_common::backends::tracing::{DeploymentLayer, DeploymentLogRecorder};
/// use shuttle_common::tracing::JsonVisitor;
/// use std::sync::{Arc, Mutex};
/// use tracing::instrument;
/// use tracing_subscriber::prelude::*;
///
/// #[derive(Default, Clone)]
/// struct RecorderMock {
///     lines: Arc<Mutex<Vec<String>>>,
/// }
///
/// impl DeploymentLogRecorder for RecorderMock {
///     fn record_log(&self, _deployment_id: &str, visitor: JsonVisitor, _metadata: &tracing::Metadata) {
///         self.lines.lock().unwrap().push(
///             visitor
///                 .fields
///                 .get("message")
///                 .unwrap()
///                 .as_str()
///                 .unwrap()
///                 .to_string(),
///         );
///     }
/// }
///
/// #[tokio::main]
/// async fn main() {
///    let recorder = RecorderMock::default();
///
///    let subscriber = tracing_subscriber::registry().with(
///        DeploymentLayer::new(recorder.clone())
///    );
///    let _guard = tracing::subscriber::set_default(subscriber);
///
///    start_deploy();
///
///    assert_eq!(
///        recorder.lines.lock().unwrap().clone(),
///        vec!["deploying", "inner"],
///        "only logs from `deploy()` and `inner()` should be captured",
///    );
/// }
///
///
/// #[instrument]
/// fn start_deploy() {
///     // This line should not be capture since it is not inside a deployment scope
///     tracing::info!("Handling deploy");
///     deploy("some_id");
/// }
///
/// #[instrument]
/// fn deploy(deployment_id: &str) {
///     // This line and everthing called by this function should be captured by this layer
///     tracing::info!("deploying");
///     inner();
/// }
///
/// #[instrument]
/// fn inner() {
///     // Since this function is called from `deploy()`, the following line should be captured
///     tracing::debug!("inner");
/// }
/// ```
pub struct DeploymentLayer<R> {
    recorder: R,
}

impl<R> DeploymentLayer<R> {
    pub fn new(recorder: R) -> Self {
        Self { recorder }
    }
}

impl<S, R> tracing_subscriber::Layer<S> for DeploymentLayer<R>
where
    S: Subscriber + for<'lookup> tracing_subscriber::registry::LookupSpan<'lookup>,
    R: DeploymentLogRecorder + Send + Sync + 'static,
{
    fn on_event(&self, event: &tracing::Event<'_>, ctx: tracing_subscriber::layer::Context<'_, S>) {
        // We only care about events in some scope
        let scope = if let Some(scope) = ctx.event_scope(event) {
            scope
        } else {
            return;
        };

        // Find the first scope with the deployment details
        for span in scope.from_root() {
            let extensions = span.extensions();

            if let Some(details) = extensions.get::<DeploymentDetails>() {
                let mut visitor = JsonVisitor::default();

                event.record(&mut visitor);
                let metadata = event.metadata();

                self.recorder.record_log(&details.id, visitor, metadata);
                break;
            }
        }
    }

    fn on_new_span(
        &self,
        attrs: &span::Attributes<'_>,
        id: &span::Id,
        ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        // We only care about spans that start a deployment context / scope
        if !DeploymentScopeVisitor::is_valid(attrs.metadata()) {
            return;
        }

        let mut visitor = DeploymentScopeVisitor::default();

        attrs.record(&mut visitor);

        if let Some(deployment_id) = visitor.id {
            // Safe to unwrap since this is the `on_new_span` method
            let span = ctx.span(id).unwrap();
            let mut extensions = span.extensions_mut();

            extensions.insert(DeploymentDetails { id: deployment_id });
        }
    }
}

/// The details of a deployment task
#[derive(Debug, Default)]
struct DeploymentDetails {
    id: String,
}

/// A visitor to extract the [DeploymentDetails] for any scope with a `deployment_id`
#[derive(Default)]
struct DeploymentScopeVisitor {
    id: Option<String>,
}

impl DeploymentScopeVisitor {
    /// Field containing the deployment identifier
    const ID_IDENT: &'static str = "deployment_id";

    fn is_valid(metadata: &Metadata) -> bool {
        metadata.is_span() && metadata.fields().field(Self::ID_IDENT).is_some()
    }
}

impl Visit for DeploymentScopeVisitor {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        if field.name() == Self::ID_IDENT {
            self.id = Some(format!("{value:?}"));
        }
    }
}
