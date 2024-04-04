use opentelemetry::{global, KeyValue};
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::{
    logs::Config, propagation::TraceContextPropagator, runtime::Tokio, trace, Resource,
};
use tracing::Subscriber;
use tracing_subscriber::{fmt, prelude::*, registry::LookupSpan, EnvFilter};

use shuttle_common::log::Backend;

use super::otlp_tracing_bridge::{self, ErrorTracingLayer};

const OTLP_ADDRESS: &str = "http://otel-collector:4317";

pub fn setup_tracing<S>(subscriber: S, backend: Backend)
where
    S: Subscriber + for<'a> LookupSpan<'a> + Send + Sync,
{
    global::set_text_map_propagator(TraceContextPropagator::new());

    let shuttle_env = std::env::var("SHUTTLE_ENV").unwrap_or_default();
    let filter_layer = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new("info"))
        .unwrap();

    let fmt_layer = fmt::layer().compact();

    // The OTLP_ADDRESS env var is useful for setting a localhost address when running deployer locally.
    let otlp_address = std::env::var("OTLP_ADDRESS").unwrap_or(OTLP_ADDRESS.into());

    let resources = {
        let mut resources = vec![
            KeyValue::new("service.name", backend.to_string().to_lowercase()),
            KeyValue::new("deployment.environment", shuttle_env.clone()),
        ];
        if let Ok(service_version) = std::env::var("SHUTTLE_SERVICE_VERSION") {
            resources.push(KeyValue::new("service.version", service_version));
        }
        resources
    };

    let tracer = opentelemetry_otlp::new_pipeline()
        .tracing()
        .with_exporter(
            opentelemetry_otlp::new_exporter()
                .tonic()
                .with_endpoint(otlp_address.clone()),
        )
        .with_trace_config(trace::config().with_resource(Resource::new(resources.clone())))
        .install_batch(Tokio)
        .unwrap();

    let otel_layer = tracing_opentelemetry::layer().with_tracer(tracer);

    let logs = opentelemetry_otlp::new_pipeline()
        .logging()
        .with_log_config(Config::default().with_resource(Resource::new(resources.clone())))
        .with_exporter(
            opentelemetry_otlp::new_exporter()
                .tonic()
                .with_endpoint(otlp_address),
        )
        .install_batch(Tokio)
        .unwrap();

    let appender_tracing_layer =
        otlp_tracing_bridge::OpenTelemetryTracingBridge::new(&logs.provider().unwrap());

    subscriber
        .with(filter_layer)
        .with(fmt_layer)
        .with(appender_tracing_layer)
        .with(otel_layer)
        // The error layer needs to go after the otel_layer, because it needs access to the
        // otel_data extension that is set on the span in the otel_layer.
        .with(ErrorTracingLayer::new())
        .init();
}
