use std::{
    collections::BTreeMap,
    marker::PhantomData,
    sync::Arc,
    time::{Duration, SystemTime},
};

use opentelemetry::{
    global,
    logs::{LogRecord as OtelLogRecord, Logger as OtelLogger, LoggerProvider as _, Severity},
    trace::{SpanId, TraceId, TracerProvider as _},
    KeyValue,
};
use opentelemetry_otlp::{WithExportConfig, OTEL_EXPORTER_OTLP_ENDPOINT};
use opentelemetry_sdk::{
    logs::{LogRecord, Logger, LoggerProvider},
    metrics::{MeterProviderBuilder, PeriodicReader, SdkMeterProvider, Temporality},
    propagation::TraceContextPropagator,
    runtime,
    trace::TracerProvider,
    Resource,
};
use opentelemetry_semantic_conventions::{
    attribute::{
        CODE_FILEPATH, CODE_LINENO, DEPLOYMENT_ENVIRONMENT_NAME, SERVICE_NAME, SERVICE_VERSION,
        TELEMETRY_SDK_LANGUAGE, TELEMETRY_SDK_NAME, TELEMETRY_SDK_VERSION,
    },
    SCHEMA_URL,
};
use tracing::Event as TracingEvent;
use tracing_core::{
    span::{Attributes, Id, Record},
    Field, Subscriber,
};
use tracing_log::AsLog;
use tracing_opentelemetry::{MetricsLayer, OpenTelemetryLayer, OtelData};
use tracing_subscriber::{
    layer::{Context, SubscriberExt},
    registry::LookupSpan,
    util::SubscriberInitExt,
    EnvFilter, Layer,
};

#[derive(Clone, Debug)]
pub struct ProviderGuard {
    logger: LoggerProvider,
    tracer: TracerProvider,
    meter: SdkMeterProvider,
}

impl Drop for ProviderGuard {
    fn drop(&mut self) {
        if let Err(error) = self.tracer.shutdown() {
            tracing::error!(%error, "Failed to shutdown tracer provider gracefully");
        }

        if let Err(error) = self.meter.shutdown() {
            tracing::error!(%error, "Failed to shutdown metrics provider gracefully");
        }

        if let Err(error) = self.logger.shutdown() {
            eprintln!(
                "ERROR - Failed to shutdown logger provider gracefully: {}",
                error
            );
        }
    }
}

trait SpanFieldVisitor {
    fn visit(&mut self, key: &'static str, value: opentelemetry::logs::AnyValue);
}

macro_rules! impl_visit {
    ($t:ty) => {
        impl tracing::field::Visit for $t {
            #[inline(always)]
            fn record_f64(&mut self, field: &Field, value: f64) {
                <Self as SpanFieldVisitor>::visit(self, field.name(), value.into());
            }

            #[inline(always)]
            fn record_i64(&mut self, field: &Field, value: i64) {
                <Self as SpanFieldVisitor>::visit(self, field.name(), value.into());
            }

            #[inline(always)]
            fn record_u64(&mut self, field: &Field, value: u64) {
                match i64::try_from(value) {
                    Ok(value) => self.record_i64(field, value),
                    Err(_) => <Self as SpanFieldVisitor>::visit(
                        self,
                        field.name(),
                        value.to_string().into(),
                    ),
                }
            }

            #[inline(always)]
            fn record_i128(&mut self, field: &Field, value: i128) {
                match i64::try_from(value) {
                    Ok(value) => self.record_i64(field, value),
                    Err(_) => <Self as SpanFieldVisitor>::visit(
                        self,
                        field.name(),
                        value.to_string().into(),
                    ),
                }
            }

            #[inline(always)]
            fn record_u128(&mut self, field: &Field, value: u128) {
                match i64::try_from(value) {
                    Ok(value) => self.record_i64(field, value),
                    Err(_) => <Self as SpanFieldVisitor>::visit(
                        self,
                        field.name(),
                        value.to_string().into(),
                    ),
                }
            }

            #[inline(always)]
            fn record_bool(&mut self, field: &Field, value: bool) {
                <Self as SpanFieldVisitor>::visit(self, field.name(), value.into());
            }

            #[inline(always)]
            fn record_str(&mut self, field: &Field, value: &str) {
                <Self as SpanFieldVisitor>::visit(self, field.name(), value.to_string().into());
            }

            #[inline(always)]
            fn record_bytes(&mut self, field: &Field, value: &[u8]) {
                <Self as SpanFieldVisitor>::visit(
                    self,
                    field.name(),
                    opentelemetry::logs::AnyValue::Bytes(Box::new(value.to_vec())),
                );
            }

            #[inline(always)]
            fn record_error(&mut self, field: &Field, value: &(dyn 'static + std::error::Error)) {
                <Self as SpanFieldVisitor>::visit(self, field.name(), value.to_string().into());
            }

            #[inline(always)]
            fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
                <Self as SpanFieldVisitor>::visit(
                    self,
                    field.name(),
                    format!("{:?}", value).into(),
                );
            }
        }
    };
}

#[derive(Clone, Debug, Default)]
struct SpanFieldValues(BTreeMap<opentelemetry::Key, opentelemetry::logs::AnyValue>);

impl SpanFieldVisitor for SpanFieldValues {
    fn visit(&mut self, key: &'static str, value: opentelemetry::logs::AnyValue) {
        self.0.insert(key.into(), value);
    }
}

impl_visit!(SpanFieldValues);

#[derive(Clone, Debug, Default)]
struct EventFieldValues {
    data: SpanFieldValues,
    message: Option<opentelemetry::logs::AnyValue>,
}

impl EventFieldValues {
    const REMAPPED_FIELDS: [(&'static str, &'static str); 4] = [
        ("log.line", CODE_LINENO),
        ("log.file", CODE_FILEPATH),
        ("log.target", "code.target"),
        ("log.module_path", "code.module_path"),
    ];
}

impl SpanFieldVisitor for EventFieldValues {
    fn visit(&mut self, key: &'static str, value: opentelemetry::logs::AnyValue) {
        // this block can be uncommented to filter out "empty" log events if needed, (i.e. events that have
        // attributes, but don't have an actual "message", e.x. events emitted by `#[tracing::instrument(ret)]`)
        // match &value {
        //     opentelemetry::logs::AnyValue::String(inner) if inner.as_str().trim().is_empty() => {
        //         return;
        //     }
        //     _ => {}
        // };

        if key == "message"
            && (self.message.is_none()
                || matches!(
                    &self.message,
                    Some(opentelemetry::logs::AnyValue::String(inner)) if inner.as_str().trim().is_empty(),
                ))
        {
            self.message = Some(value);
            return;
        }

        let key = Self::REMAPPED_FIELDS
            .iter()
            .find_map(|(bad_key, replacement)| {
                if key == *bad_key {
                    Some(*replacement)
                } else {
                    None
                }
            })
            .unwrap_or(key);

        self.data.0.insert(key.into(), value);
    }
}

impl_visit!(EventFieldValues);

#[derive(Clone, Debug)]
pub struct LogCourier<S> {
    logger: Arc<Logger>,
    marker: PhantomData<S>,
}

impl<S> LogCourier<S> {
    pub fn new(logger: Logger) -> Self {
        Self {
            logger: Arc::new(logger),
            marker: Default::default(),
        }
    }
}

impl<S> Layer<S> for LogCourier<S>
where
    S: Subscriber + std::fmt::Debug + for<'span> LookupSpan<'span>,
    for<'span> <S as LookupSpan<'span>>::Data: std::fmt::Debug,
{
    fn on_new_span(&self, attrs: &Attributes<'_>, id: &Id, ctx: Context<'_, S>) {
        let span = ctx.span(id).expect("Span not found, this is a bug");
        let mut extensions = span.extensions_mut();

        let fields = match extensions.get_mut::<SpanFieldValues>() {
            Some(fields) => fields,
            None => {
                extensions.insert(SpanFieldValues::default());
                extensions.get_mut::<SpanFieldValues>().unwrap()
            }
        };

        attrs.record(fields);
    }

    fn on_record(&self, span: &Id, values: &Record<'_>, ctx: Context<'_, S>) {
        let span = ctx.span(span).expect("Span not found, this is a bug");
        let mut extensions = span.extensions_mut();

        let fields = match extensions.get_mut::<SpanFieldValues>() {
            Some(fields) => fields,
            None => {
                extensions.insert(SpanFieldValues::default());
                extensions.get_mut::<SpanFieldValues>().unwrap()
            }
        };

        values.record(fields);
    }

    fn on_event(&self, event: &TracingEvent<'_>, ctx: Context<'_, S>) {
        let (metadata, mut record, mut attributes) = (
            event.metadata(),
            LogRecord::default(),
            EventFieldValues::default(),
        );

        event.record(&mut attributes);

        let (mut attributes, Some(message)) = (attributes.data, attributes.message) else {
            return;
        };

        record.set_body(message);
        record.set_target(metadata.target());
        record.set_event_name(metadata.name());
        record.set_timestamp(SystemTime::now());
        record.set_severity_text(metadata.level().as_str());
        record.set_severity_number(log_level_as_severity(metadata.level().as_log()));

        if let Some((trace_id, span_id)) = ctx
            .event_span(event)
            .or_else(|| ctx.lookup_current())
            .and_then(|span| {
                span.extensions_mut()
                    .get_mut::<OtelData>()
                    .and_then(|data| {
                        data.builder
                            .trace_id
                            .or(Some(TraceId::INVALID))
                            .zip(data.builder.span_id.or(Some(SpanId::INVALID)))
                    })
            })
        {
            record.set_trace_context(trace_id, span_id, None);
        }

        attributes.0.extend(
            ctx.event_scope(event)
                .into_iter()
                .flatten()
                .flat_map(
                    |span| match span.extensions_mut().get_mut::<SpanFieldValues>() {
                        Some(values) if !values.0.is_empty() => Some(values.clone().0.into_iter()),
                        _ => None,
                    },
                )
                .flatten()
                .chain(
                    [
                        metadata.file().map(|file| {
                            (
                                opentelemetry::Key::from(CODE_FILEPATH),
                                opentelemetry::logs::AnyValue::from(file.to_string()),
                            )
                        }),
                        metadata.line().map(|line| {
                            (
                                opentelemetry::Key::from(CODE_LINENO),
                                opentelemetry::logs::AnyValue::from(line),
                            )
                        }),
                        metadata.module_path().map(|path| {
                            (
                                opentelemetry::Key::from("code.module_path"),
                                opentelemetry::logs::AnyValue::from(path.to_string()),
                            )
                        }),
                    ]
                    .into_iter()
                    .flatten(),
                ),
        );

        record.add_attributes(attributes.0);

        self.logger.emit(record)
    }
}

/// Convert a [`log::Level`] to its equivalent [`Severity`]
#[inline(always)]
pub(crate) fn log_level_as_severity(level: log::Level) -> Severity {
    match level {
        log::Level::Info => Severity::Info,
        log::Level::Warn => Severity::Warn,
        log::Level::Debug => Severity::Debug,
        log::Level::Trace => Severity::Trace,
        log::Level::Error => Severity::Error,
    }
}

// Create a Resource that captures information about the entity for which telemetry is recorded.
pub fn resource(crate_name: &'static str, package_version: &'static str) -> Resource {
    let project_name = std::env::var("SHUTTLE_PROJECT_NAME").ok();

    Resource::from_schema_url(
        [
            Some(KeyValue::new(
                SERVICE_NAME,
                project_name.clone().unwrap_or_else(|| crate_name.into()),
            )),
            Some(KeyValue::new(SERVICE_VERSION, package_version)),
            Some(KeyValue::new("code.crate.name", crate_name)),
            Some(KeyValue::new(TELEMETRY_SDK_NAME, "opentelemetry")),
            Some(KeyValue::new(TELEMETRY_SDK_VERSION, "0.27.1")),
            Some(KeyValue::new(TELEMETRY_SDK_LANGUAGE, "rust")),
            std::env::var("APP_ENV")
                .ok()
                .map(|value| KeyValue::new(DEPLOYMENT_ENVIRONMENT_NAME, value)),
            Some(KeyValue::new(
                "shuttle.deployment.env",
                std::env::var("SHUTTLE_ENV")
                    .ok()
                    .unwrap_or("unknown".into()),
            )),
            std::env::var("SHUTTLE_PROJECT_ID")
                .ok()
                .map(|value| KeyValue::new("shuttle.project.id", value)),
            project_name.map(|value| KeyValue::new("shuttle.project.name", value)),
        ]
        .into_iter()
        .flatten(),
        SCHEMA_URL,
    )
}

pub fn init_log_subscriber(endpoint: &Option<String>, resource: Resource) -> LoggerProvider {
    let mut builder = LoggerProvider::builder().with_resource(resource);

    if let Some(endpoint) = endpoint {
        let exporter = opentelemetry_otlp::LogExporter::builder()
            .with_http()
            .with_endpoint(format!("{endpoint}/v1/logs"))
            .build()
            .unwrap();

        builder = builder.with_batch_exporter(exporter, runtime::Tokio);
    }

    builder.build()
}

// Construct MeterProvider for MetricsLayer
pub fn init_meter_provider(endpoint: &Option<String>, resource: Resource) -> SdkMeterProvider {
    let mut builder = MeterProviderBuilder::default().with_resource(resource);

    if let Some(endpoint) = endpoint {
        let exporter = opentelemetry_otlp::MetricExporter::builder()
            .with_temporality(Temporality::default())
            .with_http()
            .with_endpoint(format!("{endpoint}/v1/metrics"))
            .build()
            .unwrap();

        let reader = PeriodicReader::builder(exporter, runtime::Tokio)
            .with_interval(Duration::from_secs(30)) // TODO(the-wondersmith): make metrics read period configurable
            .build();

        builder = builder.with_reader(reader);
    }

    let provider = builder.build();

    global::set_meter_provider(provider.clone());

    provider
}

// Construct TracerProvider for OpenTelemetryLayer
pub fn init_tracer_provider(endpoint: &Option<String>, resource: Resource) -> TracerProvider {
    // TODO(the-wondersmith): make trace sample rate & strategy configurable
    // let sampler = opentelemetry_sdk::trace::Sampler::ParentBased(Box::new(
    //     opentelemetry_sdk::trace::Sampler::TraceIdRatioBased(1.0),
    // ));

    let mut builder = TracerProvider::builder()
        // .with_sampler(sampler)
        .with_resource(resource);

    if let Some(endpoint) = endpoint {
        let exporter = opentelemetry_otlp::SpanExporter::builder()
            .with_http()
            .with_endpoint(format!("{endpoint}/v1/traces"))
            .build()
            .unwrap();

        builder = builder.with_batch_exporter(exporter, runtime::Tokio);
    }

    let provider = builder.build();

    global::set_tracer_provider(provider.clone());

    provider
}

// Initialize tracing-subscriber and return ExporterGuard for opentelemetry-related termination processing
pub fn init_tracing_subscriber(
    crate_name: &'static str,
    package_version: &'static str,
) -> ProviderGuard {
    global::set_text_map_propagator(TraceContextPropagator::new());

    let resource = resource(crate_name, package_version);

    // The OTLP_HOST env var is useful for setting a specific host when running locally
    let endpoint = std::env::var(OTEL_EXPORTER_OTLP_ENDPOINT).ok();

    let tracer = init_tracer_provider(&endpoint, resource.clone());
    let meter = init_meter_provider(&endpoint, resource.clone());
    let logger = init_log_subscriber(&endpoint, resource);

    let level_filter =
        std::env::var("RUST_LOG").unwrap_or_else(|_| format!("info,{}=debug", crate_name));

    let layers = EnvFilter::from(&level_filter)
        .and_then(MetricsLayer::new(meter.clone()))
        .and_then(OpenTelemetryLayer::new(tracer.tracer("shuttle-telemetry")))
        .and_then(
            tracing_subscriber::fmt::layer()
                .compact()
                .with_level(true)
                .with_target(true),
        )
        .and_then(LogCourier::new(logger.logger("shuttle-telemetry")));

    tracing_subscriber::registry().with(layers).init();

    if endpoint.is_none() {
        tracing::warn!(
            "No value set for `OTEL_EXPORTER_OTLP_ENDPOINT` env var, \
            declining to attach OTLP exporter to default tracing subscriber"
        );
    }

    ProviderGuard {
        logger,
        tracer,
        meter,
    }
}
