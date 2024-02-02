// Taken from
// https://github.com/open-telemetry/opentelemetry-rust/blob/e640051b8bd5d56bb058ec6caabadf2bee5244a9/opentelemetry-appender-tracing/src/layer.rs,
// waiting for https://github.com/open-telemetry/opentelemetry-rust/pull/1394 to be merged.
// This is under Apache License 2.0

use opentelemetry::{
    logs::{AnyValue, LogRecord, Logger, LoggerProvider, Severity, TraceContext},
    trace::{SpanBuilder, SpanContext, Status, TraceFlags, TraceState},
    Key,
};
use opentelemetry::{KeyValue, StringValue};
use std::borrow::Cow;
use std::marker;
use tracing_core::{Level, Subscriber};
use tracing_opentelemetry::OtelData;
use tracing_subscriber::{registry::LookupSpan, Layer};
use valuable::{NamedField, Valuable, Value};

const INSTRUMENTATION_LIBRARY_NAME: &str = "opentelemetry-appender-tracing";

pub static ERROR_FIELDS: &[NamedField<'static>] = &[
    NamedField::new("message"),
    NamedField::new("stack"),
    NamedField::new("type"),
];

type OtlpTag = (Key, AnyValue);
struct VisitError {
    field: String,
    errors: Vec<OtlpTag>,
}

struct VisitErrorRepr<'a> {
    field: String,
    errors: &'a mut Vec<OtlpTag>,
}

impl<'a> valuable::Visit for VisitErrorRepr<'a> {
    fn visit_value(&mut self, value: Value<'_>) {
        match value {
            valuable::Value::String(s) => self
                .errors
                .push((self.field.to_string().into(), s.to_string().into())),
            _ => {}
        }
    }
}

impl valuable::Visit for VisitError {
    fn visit_value(&mut self, value: valuable::Value<'_>) {
        match value {
            valuable::Value::Structable(v) => v.visit(self),
            _ => {}
        }
    }

    fn visit_named_fields(&mut self, named_values: &valuable::NamedValues<'_>) {
        for field in ERROR_FIELDS {
            let mut visit = VisitErrorRepr {
                field: format!("{}.{}", self.field, field.name()),
                errors: &mut self.errors,
            };
            match named_values.get_by_name(field.name()) {
                Some(Value::String(s)) => {
                    s.visit(&mut visit);
                }
                _ => {}
            }
        }
    }
}

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

    fn record_error(
        &mut self,
        field: &tracing_core::Field,
        value: &(dyn std::error::Error + 'static),
    ) {
        println!("called for {field}");
        let chain = {
            let mut chain: Vec<StringValue> = Vec::new();
            let mut next_err = value.source();

            while let Some(err) = next_err {
                chain.push(err.to_string().into());
                next_err = err.source();
            }
            opentelemetry::Value::Array(chain.into())
        };

        let error_msg = value.to_string();

        if let Some(ref mut vec) = self.log_record.attributes {
            // vec.push((field.name().into(), error_msg.clone().into()));
            vec.push(("error.message".into(), error_msg.into()));
            vec.push(("error.stack".into(), chain.into()));
            vec.push(("error.kind".into(), "error".into()));
        } else {
            let vec = vec![
                // (field.name().into(), error_msg.clone().into()),
                ("error.message".into(), error_msg.into()),
                ("error.stack".into(), chain.into()),
                ("error.kind".into(), "error".into()),
            ];
            self.log_record.attributes = Some(vec);
        }
    }

    fn record_value(&mut self, field: &tracing_core::Field, value: valuable::Value<'_>) {
        if field.name() == "error" {
            let mut visit = VisitError {
                field: field.name().to_string(),
                errors: vec![],
            };
            valuable::visit(&value, &mut visit);
            if let Some(ref mut vec) = self.log_record.attributes {
                vec.extend_from_slice(&visit.errors[..]);
            } else {
                self.log_record.attributes = Some(visit.errors);
            }
            println!("{:?}", self.log_record.attributes);
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

struct ErrorSpanRecord<'a> {
    field: String,
    builder: &'a mut SpanBuilder,
}

fn record(builder: &mut SpanBuilder, attribute: KeyValue) {
    debug_assert!(builder.attributes.is_some());
    if let Some(v) = builder.attributes.as_mut() {
        v.push(KeyValue::new(attribute.key, attribute.value));
    }
}

struct ErrorSpanAttributeVisitor<'a> {
    field: String,
    builder: &'a mut SpanBuilder,
}
impl<'a> valuable::Visit for ErrorSpanAttributeVisitor<'a> {
    fn visit_value(&mut self, value: Value<'_>) {
        if let valuable::Value::String(s) = value {
            record(
                self.builder,
                KeyValue::new(self.field.to_string(), s.to_string()),
            )
        }
    }
}

impl<'a> valuable::Visit for ErrorSpanRecord<'a> {
    fn visit_value(&mut self, value: valuable::Value<'_>) {
        if let valuable::Value::Structable(v) = value {
            v.visit(self)
        }
    }

    fn visit_named_fields(&mut self, named_values: &valuable::NamedValues<'_>) {
        for field in ERROR_FIELDS {
            let mut visit = ErrorSpanAttributeVisitor {
                field: format!("{}.{}", self.field, field.name()),
                builder: self.builder,
            };
            if let Some(Value::String(s)) = named_values.get_by_name(field.name()) {
                s.visit(&mut visit);
            }
        }
    }
}

/// Visitor to record the fields from the event record.
struct ErrorEventVisitor<'a> {
    builder: &'a mut SpanBuilder,
}

pub fn build_source_chain(error: &(dyn std::error::Error + 'static)) -> String {
    let chain = {
        let mut chain: Vec<StringValue> = Vec::new();
        let mut next_err = error.source();

        while let Some(err) = next_err {
            chain.push(err.to_string().into());
            next_err = err.source();
        }
        opentelemetry::Value::Array(chain.into())
    };

    chain.to_string()
}

impl<'a> tracing::field::Visit for ErrorEventVisitor<'a> {
    fn record_error(
        &mut self,
        _field: &tracing_core::Field,
        value: &(dyn std::error::Error + 'static),
    ) {
        // TODO: do we want to do something with the field's name ?
        let error_msg = value.to_string();

        record(self.builder, KeyValue::new("error.message", error_msg));
        record(
            self.builder,
            KeyValue::new("error.stack", build_source_chain(value)),
        );
        record(
            self.builder,
            KeyValue::new("error.type", "Error".to_string()),
        );
    }

    fn record_value(&mut self, field: &tracing_core::Field, value: valuable::Value<'_>) {
        // Should we try to "duck-type" every kind of error ? Probably not since we want a single
        // error field, I think this is fine as a convention.
        if field.name() == "error" {
            self.builder.status = Status::error("");
            // if let Some(attr) = self.builder.attributes.as_mut() {
            //     if let Some(key_val) = attr.last() {
            //         if key_val.key.as_str() == "error" {
            //             attr.pop();
            //         }
            //     }
            // }
            let mut visit = ErrorSpanRecord {
                field: field.name().to_string(),
                builder: self.builder,
            };
            valuable::visit(&value, &mut visit);
            println!("Builder is now {:?}", self.builder);
        }
    }

    fn record_debug(&mut self, _field: &tracing_core::Field, _value: &dyn std::fmt::Debug) {
        // Don't do anything when recording a debug field. This is taken care of by other layers.
    }
}

pub struct ErrorTracingLayer<S> {
    _registry: marker::PhantomData<S>,
}

impl<S> ErrorTracingLayer<S>
where
    S: Subscriber + for<'span> LookupSpan<'span>,
{
    pub fn new() -> Self {
        ErrorTracingLayer {
            _registry: marker::PhantomData,
        }
    }
}

impl<S> Layer<S> for ErrorTracingLayer<S>
where
    S: Subscriber + for<'span> LookupSpan<'span>,
{
    fn on_new_span(
        &self,
        attrs: &tracing_core::span::Attributes<'_>,
        id: &tracing_core::span::Id,
        ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let span = ctx.span(id).expect("Span not found, this is a bug");
        let mut extensions = span.extensions_mut();

        extensions.insert(ErrorData { had_error: false });

        if let Some(otel_data) = extensions.get_mut::<OtelData>() {
            let prev_length = otel_data
                .builder
                .attributes
                .as_ref()
                .map(|v| v.len())
                .unwrap_or(0);
            let mut visitor = ErrorEventVisitor {
                builder: &mut otel_data.builder,
            };
            attrs.record(&mut visitor);
            // if otel_data
            //     .builder
            //     .attributes
            //     .as_ref()
            //     .map(|v| v.len())
            //     .unwrap_or(0)
            //     != prev_length
            // {
            //     let ext = extensions
            //         .get_mut::<ErrorData>()
            //         .expect("We've just inserted it");
            //     ext.had_error = true;
            //     println!("{:?}", ext);
            // }
        }
    }

    fn on_record(
        &self,
        id: &tracing_core::span::Id,
        values: &tracing_core::span::Record<'_>,
        ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let span = ctx.span(id).expect("Span not found, this is a bug");
        let mut extensions = span.extensions_mut();
        if let Some(ErrorData { had_error: true }) = extensions.get_mut::<ErrorData>() {
            println!("already recorded an error, aborting");
            return;
        }
        if let Some(otel_data) = extensions.get_mut::<OtelData>() {
            let prev_length = otel_data
                .builder
                .attributes
                .as_ref()
                .map(|v| v.len())
                .unwrap_or(0);
            let mut visitor = ErrorEventVisitor {
                builder: &mut otel_data.builder,
            };
            values.record(&mut visitor);
            if otel_data
                .builder
                .attributes
                .as_ref()
                .map(|v| v.len())
                .unwrap_or(0)
                != prev_length
            {
                let ext = extensions
                    .get_mut::<ErrorData>()
                    .expect("We've just inserted it");
                ext.had_error = true;
                println!("{:?}", ext);
            }
        }
    }
}

#[derive(Debug)]
struct ErrorData {
    had_error: bool,
}
