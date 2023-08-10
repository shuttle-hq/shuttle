use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

use chrono::Utc;
use http::{Request, Response};
use opentelemetry::{
    global,
    runtime::Tokio,
    sdk::{propagation::TraceContextPropagator, trace, Resource},
    KeyValue,
};
use opentelemetry_http::HeaderExtractor;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_proto::tonic::{
    collector::logs::v1::{logs_service_client::LogsServiceClient, ExportLogsServiceRequest},
    common::v1::{any_value, AnyValue, InstrumentationScope},
    logs::v1::{LogRecord, ResourceLogs, ScopeLogs, SeverityNumber},
};
use pin_project::pin_project;
use tokio::sync::mpsc;
use tower::{Layer, Service};
use tracing::{
    debug_span, error, field::Visit, instrument::Instrumented, span, Instrument, Level, Metadata,
    Span, Subscriber,
};
use tracing_opentelemetry::OpenTelemetrySpanExt;
use tracing_subscriber::{fmt, prelude::*, registry::LookupSpan, EnvFilter};

use crate::tracing::{
    JsonVisitor, FILEPATH_KEY, LINENO_KEY, MESSAGE_KEY, NAMESPACE_KEY, TARGET_KEY,
};

const OTLP_ADDRESS: &str = "http://otel-collector:4317";
const LOGGER_URI: &str = "http://logger:8009";

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
                .with_endpoint(OTLP_ADDRESS),
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

    let deployment_layer = if service_name != "logger" {
        let logger_address = std::env::var("LOGGER_URI").unwrap_or(LOGGER_URI.to_string());

        println!("connecting to logger at address: {logger_address}");

        Some(DeploymentLayer::new(OtlpDeploymentLogRecorder::new(
            service_name,
            &logger_address,
        )))
    } else {
        None
    };

    subscriber
        .with(filter_layer)
        .with(fmt_layer)
        .with(otel_layer)
        .with(deployment_layer)
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
    fn record_log(&self, details: &DeploymentDetails, visitor: JsonVisitor, metadata: &Metadata);
}

/// Recorder to send deployment logs over OTLP
pub struct OtlpDeploymentLogRecorder {
    tx: mpsc::UnboundedSender<ScopeLogs>,
}

impl OtlpDeploymentLogRecorder {
    /// Send deployment logs to `destination`. Also mark all logs as being generated by the `service_name`
    pub fn new(service_name: &str, destination: &str) -> Self {
        let destination = destination.to_string();
        let (tx, mut rx) = mpsc::unbounded_channel();

        let resource_attributes = vec![("service.name".into(), service_name.into())];
        let resource_attributes =
            serde_json_map_to_key_value_list(serde_json::Map::from_iter(resource_attributes));

        let resource = Some(opentelemetry_proto::tonic::resource::v1::Resource {
            attributes: resource_attributes,
            ..Default::default()
        });

        tokio::spawn(async move {
            let mut otlp_client = match LogsServiceClient::connect(destination).await {
                Ok(client) => client,
                Err(error) => {
                    error!(
                        error = &error as &dyn std::error::Error,
                        "Could not connect to OTLP collector for logs. No logs will be send"
                    );

                    return;
                }
            };

            while let Some(scope_logs) = rx.recv().await {
                let resource_log = ResourceLogs {
                    scope_logs: vec![scope_logs],
                    resource: resource.clone(),
                    ..Default::default()
                };
                let request = tonic::Request::new(ExportLogsServiceRequest {
                    resource_logs: vec![resource_log],
                });

                if let Err(error) = otlp_client.export(request).await {
                    error!(
                        error = &error as &dyn std::error::Error,
                        "Otlp deployment log recorder encountered error while exporting the logs"
                    );
                };
            }
        });
        Self { tx }
    }
}

impl DeploymentLogRecorder for OtlpDeploymentLogRecorder {
    fn record_log(&self, details: &DeploymentDetails, visitor: JsonVisitor, metadata: &Metadata) {
        let log_record = into_log_record(visitor, metadata);

        let scope_attributes = vec![("deployment_id".into(), details.id.clone().into())];
        let scope_attributes =
            serde_json_map_to_key_value_list(serde_json::Map::from_iter(scope_attributes));

        let scope_logs = ScopeLogs {
            scope: Some(InstrumentationScope {
                name: details.name.to_string(),
                attributes: scope_attributes,
                ..Default::default()
            }),
            log_records: vec![log_record],
            ..Default::default()
        };

        if let Err(error) = self.tx.send(scope_logs) {
            error!(
                error = &error as &dyn std::error::Error,
                "Failed to send deployment log in recorder"
            );
        }
    }
}

/// Tracing layer to capture logs that relate to a deployment task.
///
/// This causes any functions instrumented with the `deployment_id` attribute to have its logs associated with the
/// deployment. Thus, the instrument span acts as the context for logs to capture.
///
/// # Example
/// ```
/// use shuttle_common::backends::tracing::{DeploymentLayer, DeploymentLogRecorder, DeploymentDetails};
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
///     fn record_log(&self, _details: &DeploymentDetails, visitor: JsonVisitor, _metadata: &tracing::Metadata) {
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

                self.recorder.record_log(details, visitor, metadata);
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

            extensions.insert(DeploymentDetails {
                name: attrs.metadata().name().to_string(),
                id: deployment_id,
            });
        }
    }
}

/// The details of a deployment task
#[derive(Debug, Default)]
pub struct DeploymentDetails {
    name: String,
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
    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        if field.name() == Self::ID_IDENT {
            self.id = Some(value.to_string());
        }
    }
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        if field.name() == Self::ID_IDENT {
            self.id = Some(format!("{value:?}"));
        }
    }
}

/// Use metadata to turn self into a [LogRecord]
pub fn into_log_record(mut visitor: JsonVisitor, metadata: &Metadata) -> LogRecord {
    let body = get_body(&mut visitor);
    let severity_number = get_severity_number(metadata);
    let attributes = enrich_with_metadata(visitor, metadata);
    let attributes = serde_json_map_to_key_value_list(attributes);

    LogRecord {
        time_unix_nano: Utc::now().timestamp_nanos() as u64,
        severity_number: severity_number.into(),
        severity_text: metadata.level().to_string(),
        body: serde_json_value_to_any_value(body),
        attributes,
        dropped_attributes_count: 0,
        ..Default::default()
    }
}

/// Get the body from a visitor
fn get_body(visitor: &mut JsonVisitor) -> serde_json::Value {
    visitor.fields.remove(MESSAGE_KEY).unwrap_or_default()
}

/// Add metadata information to own fields and return those fields
fn enrich_with_metadata(
    visitor: JsonVisitor,
    metadata: &Metadata,
) -> serde_json::Map<String, serde_json::Value> {
    let JsonVisitor {
        mut fields,
        target,
        file,
        line,
    } = visitor;

    fields.insert(
        TARGET_KEY.to_string(),
        serde_json::Value::String(target.unwrap_or(metadata.target().to_string())),
    );

    if let Some(filepath) = file.or(metadata.file().map(ToString::to_string)) {
        fields.insert(
            FILEPATH_KEY.to_string(),
            serde_json::Value::String(filepath),
        );
    }

    if let Some(lineno) = line.or(metadata.line()) {
        fields.insert(
            LINENO_KEY.to_string(),
            serde_json::Value::Number(lineno.into()),
        );
    }

    if let Some(namespace) = metadata.module_path() {
        fields.insert(
            NAMESPACE_KEY.to_string(),
            serde_json::Value::String(namespace.to_string()),
        );
    }

    fields
}

fn get_severity_number(metadata: &Metadata) -> SeverityNumber {
    match *metadata.level() {
        Level::TRACE => SeverityNumber::Trace,
        Level::DEBUG => SeverityNumber::Debug,
        Level::INFO => SeverityNumber::Info,
        Level::WARN => SeverityNumber::Warn,
        Level::ERROR => SeverityNumber::Error,
    }
}

fn serde_json_value_to_any_value(
    value: serde_json::Value,
) -> Option<opentelemetry_proto::tonic::common::v1::AnyValue> {
    use opentelemetry_proto::tonic::common::v1::any_value::Value;

    let value = match value {
        serde_json::Value::Null => return None,
        serde_json::Value::Bool(b) => Value::BoolValue(b),
        serde_json::Value::Number(n) => {
            if n.is_f64() {
                Value::DoubleValue(n.as_f64().unwrap()) // Safe to unwrap as we just checked if it is a f64
            } else {
                Value::IntValue(n.as_i64().unwrap()) // Safe to unwrap since we know it is not a f64
            }
        }
        serde_json::Value::String(s) => Value::StringValue(s),
        serde_json::Value::Array(a) => {
            Value::ArrayValue(opentelemetry_proto::tonic::common::v1::ArrayValue {
                values: a
                    .into_iter()
                    .flat_map(serde_json_value_to_any_value)
                    .collect(),
            })
        }
        serde_json::Value::Object(o) => {
            Value::KvlistValue(opentelemetry_proto::tonic::common::v1::KeyValueList {
                values: serde_json_map_to_key_value_list(o),
            })
        }
    };

    Some(opentelemetry_proto::tonic::common::v1::AnyValue { value: Some(value) })
}

/// Convert a [serde_json::Map] into an anyvalue [KeyValue] list
pub fn serde_json_map_to_key_value_list(
    map: serde_json::Map<String, serde_json::Value>,
) -> Vec<opentelemetry_proto::tonic::common::v1::KeyValue> {
    map.into_iter()
        .map(
            |(key, value)| opentelemetry_proto::tonic::common::v1::KeyValue {
                key,
                value: serde_json_value_to_any_value(value),
            },
        )
        .collect()
}

/// Convert an [AnyValue] to a [serde_json::Value]
pub fn from_any_value_to_serde_json_value(any_value: AnyValue) -> serde_json::Value {
    let Some(value) = any_value.value else {
        return serde_json::Value::Null
    };

    match value {
        any_value::Value::StringValue(s) => serde_json::Value::String(s),
        any_value::Value::BoolValue(b) => serde_json::Value::Bool(b),
        any_value::Value::IntValue(i) => serde_json::Value::Number(i.into()),
        any_value::Value::DoubleValue(f) => {
            let Some(number) = serde_json::Number::from_f64(f) else {return serde_json::Value::Null};
            serde_json::Value::Number(number)
        }
        any_value::Value::ArrayValue(a) => {
            let values = a
                .values
                .into_iter()
                .map(from_any_value_to_serde_json_value)
                .collect();

            serde_json::Value::Array(values)
        }
        any_value::Value::KvlistValue(kv) => {
            let map = from_any_value_kv_to_serde_json_map(kv.values);

            serde_json::Value::Object(map)
        }
        any_value::Value::BytesValue(_) => serde_json::Value::Null,
    }
}

/// Convert a [KeyValue] list in a [serde_json::Map]
pub fn from_any_value_kv_to_serde_json_map(
    kv_list: Vec<opentelemetry_proto::tonic::common::v1::KeyValue>,
) -> serde_json::Map<String, serde_json::Value> {
    let iter = kv_list
        .into_iter()
        .flat_map(|kv| Some((kv.key, from_any_value_to_serde_json_value(kv.value?))));

    serde_json::Map::from_iter(iter)
}
