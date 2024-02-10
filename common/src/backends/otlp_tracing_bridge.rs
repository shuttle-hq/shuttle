// Taken from
// https://github.com/open-telemetry/opentelemetry-rust/blob/e640051b8bd5d56bb058ec6caabadf2bee5244a9/opentelemetry-appender-tracing/src/layer.rs,
// waiting for https://github.com/open-telemetry/opentelemetry-rust/pull/1394 to be merged.
// This is under Apache License 2.0

use opentelemetry::{
    logs::{LogRecord, Logger, LoggerProvider, Severity, TraceContext},
    trace::{SpanContext, TraceFlags, TraceState},
    KeyValue,
};
use std::borrow::Cow;
use tracing_core::{field::Visit, Field, Level, Metadata, Subscriber};
use tracing_opentelemetry::OtelData;
use tracing_subscriber::{registry::LookupSpan, Layer};

use crate::models::error::DatadogError;

const INSTRUMENTATION_LIBRARY_NAME: &str = "opentelemetry-appender-tracing";

/// Visitor to record the fields from the event record.
struct EventVisitor<'a> {
    log_record: &'a mut LogRecord,
}

impl<'a> tracing::field::Visit for EventVisitor<'a> {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            self.log_record.body = Some(format!("{value:?}").into());
        } else if let Some(ref mut vec) = self.log_record.attributes {
            vec.push((field.name().into(), format!("{value:?}").into()));
        } else {
            let vec = vec![(field.name().into(), format!("{value:?}").into())];
            self.log_record.attributes = Some(vec);
        }
    }

    fn record_str(&mut self, field: &tracing_core::Field, value: &str) {
        if let Some(ref mut vec) = self.log_record.attributes {
            vec.push((field.name().into(), value.to_owned().into()));
        } else {
            let vec = vec![(field.name().into(), value.to_owned().into())];
            self.log_record.attributes = Some(vec);
        }
    }

    fn record_bool(&mut self, field: &tracing_core::Field, value: bool) {
        if let Some(ref mut vec) = self.log_record.attributes {
            vec.push((field.name().into(), value.into()));
        } else {
            let vec = vec![(field.name().into(), value.into())];
            self.log_record.attributes = Some(vec);
        }
    }

    fn record_f64(&mut self, field: &tracing::field::Field, value: f64) {
        if let Some(ref mut vec) = self.log_record.attributes {
            vec.push((field.name().into(), value.into()));
        } else {
            let vec = vec![(field.name().into(), value.into())];
            self.log_record.attributes = Some(vec);
        }
    }

    fn record_i64(&mut self, field: &tracing::field::Field, value: i64) {
        if let Some(ref mut vec) = self.log_record.attributes {
            vec.push((field.name().into(), value.into()));
        } else {
            let vec = vec![(field.name().into(), value.into())];
            self.log_record.attributes = Some(vec);
        }
    }
}

pub struct OpenTelemetryTracingBridge<P, L>
where
    P: LoggerProvider<Logger = L> + Send + Sync,
    L: Logger + Send + Sync,
{
    logger: L,
    _phantom: std::marker::PhantomData<P>, // P is not used.
}

impl<P, L> OpenTelemetryTracingBridge<P, L>
where
    P: LoggerProvider<Logger = L> + Send + Sync,
    L: Logger + Send + Sync,
{
    pub fn new(provider: &P) -> Self {
        OpenTelemetryTracingBridge {
            logger: provider.versioned_logger(
                INSTRUMENTATION_LIBRARY_NAME,
                Some(Cow::Borrowed(env!("CARGO_PKG_VERSION"))),
                None,
                None,
            ),
            _phantom: Default::default(),
        }
    }
}

impl<S, P, L> Layer<S> for OpenTelemetryTracingBridge<P, L>
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    P: LoggerProvider<Logger = L> + Send + Sync + 'static,
    L: Logger + Send + Sync + 'static,
{
    fn on_event(&self, event: &tracing::Event<'_>, ctx: tracing_subscriber::layer::Context<'_, S>) {
        let meta = event.metadata();
        let mut log_record: LogRecord = LogRecord::default();
        log_record.severity_number = Some(severity_of_level(meta.level()));
        log_record.severity_text = Some(meta.level().to_string().into());

        // Extract the trace_id & span_id from the opentelemetry extension.
        if let Some((trace_id, span_id)) = ctx.lookup_current().and_then(|span| {
            span.extensions()
                .get::<OtelData>()
                .and_then(|ext| ext.builder.trace_id.zip(ext.builder.span_id))
        }) {
            log_record.trace_context = Some(TraceContext::from(&SpanContext::new(
                trace_id,
                span_id,
                TraceFlags::default(),
                false,
                TraceState::default(),
            )));
        }

        // add the `name` metadata to attributes
        // TBD - Propose this to be part of log_record metadata.
        let vec = vec![
            ("level", meta.level().to_string()),
            ("target", meta.target().to_string()),
        ];
        log_record.attributes = Some(vec.into_iter().map(|(k, v)| (k.into(), v.into())).collect());

        // Not populating ObservedTimestamp, instead relying on OpenTelemetry
        // API to populate it with current time.

        let mut visitor = EventVisitor {
            log_record: &mut log_record,
        };
        event.record(&mut visitor);
        self.logger.emit(log_record);
    }
}

const fn severity_of_level(level: &Level) -> Severity {
    match *level {
        Level::TRACE => Severity::Trace,
        Level::DEBUG => Severity::Debug,
        Level::INFO => Severity::Info,
        Level::WARN => Severity::Warn,
        Level::ERROR => Severity::Error,
    }
}

pub struct ErrorTracingLayer<S> {
    _registry: std::marker::PhantomData<S>,
}

impl<S> ErrorTracingLayer<S>
where
    S: Subscriber + for<'span> LookupSpan<'span>,
{
    pub fn new() -> Self {
        ErrorTracingLayer {
            _registry: std::marker::PhantomData,
        }
    }
}

impl<S> Layer<S> for ErrorTracingLayer<S>
where
    S: Subscriber + for<'span> LookupSpan<'span>,
{
    fn on_event(
        &self,
        event: &tracing_core::Event<'_>,
        ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        // We only care about events that have the json datadog error.
        if *event.metadata().level() != Level::ERROR
            || !ErrorJsonVisitor::is_valid(event.metadata())
        {
            return;
        }

        let mut visitor = ErrorJsonVisitor::default();
        event.record(&mut visitor);
        let error = visitor.error;

        println!("looking for span");
        if let Some(span) = ctx.lookup_current() {
            println!("found span in event lookup");
            let mut extensions = span.extensions_mut();
            if let Some(otel_data) = extensions.get_mut::<OtelData>() {
                println!("found otel data in span span");
                let error_fields = [
                    KeyValue::new("error.message", error.message),
                    KeyValue::new("error.type", error.r#type),
                    KeyValue::new("error.stack", error.stack),
                ];
                let builder_attrs = otel_data
                    .builder
                    .attributes
                    .get_or_insert(Vec::with_capacity(3));
                builder_attrs.extend(error_fields);
                println!("otel data after insert: {:?}", builder_attrs);
            }
        };
    }
}

#[derive(Default)]
struct ErrorJsonVisitor {
    error: DatadogError,
}

impl ErrorJsonVisitor {
    const ID_IDENT: &'static str = "dd_error";

    fn is_valid(metadata: &Metadata) -> bool {
        metadata.is_event() && metadata.fields().field(Self::ID_IDENT).is_some()
    }
}

impl Visit for ErrorJsonVisitor {
    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        if field.name() == Self::ID_IDENT {
            self.error = serde_json::from_str(value).unwrap_or_default();
        }
    }
    fn record_debug(&mut self, _field: &Field, _value: &dyn std::fmt::Debug) {}
}
