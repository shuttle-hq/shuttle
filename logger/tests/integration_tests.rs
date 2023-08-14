use std::net::{Ipv4Addr, SocketAddr};

use opentelemetry_otlp::WithExportConfig;
use opentelemetry_proto::tonic::collector::trace::v1::trace_service_server::TraceServiceServer;
use portpicker::pick_unused_port;
use pretty_assertions::assert_eq;
use serde_json::{json, Value};
use shuttle_common::{
    claims::Scope,
    tracing::{FILEPATH_KEY, LINENO_KEY, NAMESPACE_KEY, TARGET_KEY},
};
use shuttle_common_tests::JwtScopesLayer;
use shuttle_logger::{Service, ShuttleLogsOtlp, Sqlite};
use shuttle_proto::logger::{
    logger_client::LoggerClient, logger_server::LoggerServer, LogItem, LogLevel, LogsRequest,
};
use tokio::{select, time::timeout};
use tonic::{transport::Server, Request};
use tracing::{
    debug, error, info, instrument,
    span::{self, Attributes},
    trace, warn, Id, Subscriber,
};
use tracing_subscriber::Layer;
use tracing_subscriber::{layer::Context, prelude::*, registry::LookupSpan};

// struct DeployLayer;
// impl<S> Layer<S> for DeployLayer
// where
//     S: Subscriber + for<'lookup> LookupSpan<'lookup>,
// {
//     fn on_new_span(&self, attrs: &Attributes<'_>, id: &Id, ctx: Context<'_, S>) {

//     }
// }

#[tokio::test]
async fn fetch_logs() {
    let port = pick_unused_port().unwrap();
    let addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), port);
    const DEPLOYMENT_ID: &str = "fetch-logs-deployment-id";

    let server_future = async move {
        let sqlite = Sqlite::new_in_memory().await;

        Server::builder()
            .layer(JwtScopesLayer::new(vec![Scope::Logs]))
            .add_service(TraceServiceServer::new(ShuttleLogsOtlp::new(
                sqlite.get_sender(),
            )))
            .add_service(LoggerServer::new(Service::new(sqlite.get_sender(), sqlite)))
            .serve(addr)
            .await
            .unwrap()
    };

    tokio::task::spawn(server_future);

    let test_future = async move {
        // Make sure the server starts first
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        let tracer = opentelemetry_otlp::new_pipeline()
            .tracing()
            .with_exporter(
                opentelemetry_otlp::new_exporter()
                    .tonic()
                    .with_endpoint(format!("http://127.0.0.1:{port}")),
            )
            .with_trace_config(opentelemetry::sdk::trace::config().with_resource(
                opentelemetry::sdk::Resource::new(vec![opentelemetry::KeyValue::new(
                    "service.name",
                    "test",
                )]),
            ))
            .install_simple()
            .unwrap();
        let otel_layer = tracing_opentelemetry::layer().with_tracer(tracer);

        let _guard = tracing_subscriber::registry()
            .with(otel_layer)
            .set_default();

        // Generate some logs
        deploy(DEPLOYMENT_ID.into());
    };

    tokio::task::spawn(test_future);

    tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
    let dst = format!("http://localhost:{port}");

    let mut client = LoggerClient::connect(dst).await.unwrap();

    // Get the generated logs
    let response = client
        .get_logs(Request::new(LogsRequest {
            deployment_id: DEPLOYMENT_ID.into(),
        }))
        .await
        .unwrap()
        .into_inner();

    let expected = vec![
        MinLogItem {
            level: LogLevel::Error,
            fields: json!({"message": "error"}),
        },
        MinLogItem {
            level: LogLevel::Warn,
            fields: json!({"message": "warn"}),
        },
        MinLogItem {
            level: LogLevel::Info,
            fields: json!({"message": "info", "deployment_id": DEPLOYMENT_ID.to_string()}),
        },
        MinLogItem {
            level: LogLevel::Debug,
            fields: json!({"message": "debug"}),
        },
        MinLogItem {
            level: LogLevel::Trace,
            fields: json!({"message": "trace"}),
        },
    ];

    assert_eq!(
        response
            .log_items
            .into_iter()
            .map(MinLogItem::from)
            .collect::<Vec<_>>(),
        expected
    );
}

#[tokio::test]
async fn stream_logs() {
    let port = pick_unused_port().unwrap();
    let addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), port);
    const DEPLOYMENT_ID: &str = "stream-logs-deployment-id";

    let server_future = async move {
        let sqlite = Sqlite::new_in_memory().await;

        Server::builder()
            .layer(JwtScopesLayer::new(vec![Scope::Logs]))
            .add_service(TraceServiceServer::new(ShuttleLogsOtlp::new(
                sqlite.get_sender(),
            )))
            .add_service(LoggerServer::new(Service::new(sqlite.get_sender(), sqlite)))
            .serve(addr)
            .await
            .unwrap()
    };

    tokio::spawn(server_future);

    let test_future = async move {
        // Make sure the server starts first
        tokio::time::sleep(std::time::Duration::from_millis(1000)).await;

        let tracer = opentelemetry_otlp::new_pipeline()
            .tracing()
            .with_exporter(
                opentelemetry_otlp::new_exporter()
                    .tonic()
                    .with_endpoint(format!("http://127.0.0.1:{port}")),
            )
            .with_trace_config(opentelemetry::sdk::trace::config().with_resource(
                opentelemetry::sdk::Resource::new(vec![opentelemetry::KeyValue::new(
                    "service.name",
                    "test",
                )]),
            ))
            .install_simple()
            .unwrap();
        let otel_layer = tracing_opentelemetry::layer().with_tracer(tracer);

        let _guard = tracing_subscriber::registry()
            .with(otel_layer)
            .set_default();

        // Generate some logs
        foo(DEPLOYMENT_ID.into());
    };

    tokio::spawn(test_future);
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let dst = format!("http://localhost:{port}");

    let mut client = LoggerClient::connect(dst).await.unwrap();

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // Subscribe to stream
    let mut response = client
        .get_logs_stream(Request::new(LogsRequest {
            deployment_id: DEPLOYMENT_ID.into(),
        }))
        .await
        .unwrap()
        .into_inner();

    let log = timeout(std::time::Duration::from_millis(500), response.message())
        .await
        .unwrap()
        .unwrap()
        .unwrap();

    assert_eq!(
        MinLogItem::from(log),
        MinLogItem {
            level: LogLevel::Trace,
            fields: json!({"message": "foo"}),
        },
    );

    // Generate some more logs
    let test_future = async move {
        let tracer = opentelemetry_otlp::new_pipeline()
            .tracing()
            .with_exporter(
                opentelemetry_otlp::new_exporter()
                    .tonic()
                    .with_endpoint(format!("http://127.0.0.1:{port}")),
            )
            .with_trace_config(opentelemetry::sdk::trace::config().with_resource(
                opentelemetry::sdk::Resource::new(vec![opentelemetry::KeyValue::new(
                    "service.name",
                    "test",
                )]),
            ))
            .install_simple()
            .unwrap();
        let otel_layer = tracing_opentelemetry::layer().with_tracer(tracer);

        let _guard = tracing_subscriber::registry()
            .with(otel_layer)
            .set_default();

        // Generate some logs
        bar(DEPLOYMENT_ID.into());
    };

    tokio::spawn(test_future);
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let log = timeout(std::time::Duration::from_millis(500), response.message())
        .await
        .unwrap()
        .unwrap()
        .unwrap();

    assert_eq!(
        MinLogItem::from(log),
        MinLogItem {
            level: LogLevel::Trace,
            fields: json!({"message": "bar"}),
        },
    );
}

#[instrument(fields(%deployment_id))]
fn deploy(deployment_id: String) {
    error!("error");
    warn!("warn");
    info!(%deployment_id, "info");
    debug!("debug");
    trace!("trace");
}

#[instrument(fields(%deployment_id))]
fn foo(deployment_id: String) {
    trace!("foo");
}

#[instrument(fields(%deployment_id))]
fn bar(deployment_id: String) {
    trace!("bar");
}

#[derive(Debug, Eq, PartialEq)]
struct MinLogItem {
    level: LogLevel,
    fields: Value,
}

impl From<LogItem> for MinLogItem {
    fn from(log: LogItem) -> Self {
        assert_eq!(log.service_name, "test");

        let fields = if log.fields.is_empty() {
            Value::Null
        } else {
            let mut fields: Value = serde_json::from_slice(&log.fields).unwrap();

            let map = fields.as_object_mut().unwrap();
            let target = map.remove(TARGET_KEY).unwrap();
            let filepath = map.remove(FILEPATH_KEY).unwrap();

            assert_eq!(target, "integration_tests");
            assert_eq!(filepath, "logger/tests/integration_tests.rs");

            map.remove(LINENO_KEY).unwrap();
            map.remove(NAMESPACE_KEY).unwrap();

            fields
        };

        Self {
            level: log.level(),
            fields,
        }
    }
}
