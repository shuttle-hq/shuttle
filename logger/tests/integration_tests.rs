use std::net::{Ipv4Addr, SocketAddr};

use opentelemetry::KeyValue;
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
use tokio::time::timeout;
use tonic::{transport::Server, Request};
use tracing::{debug, error, info, instrument, trace, warn};
use tracing_subscriber::prelude::*;

// TODO: find out why these tests affect one-another. If running them together setting the timeouts
// low will cause them to fail spuriously. If running single tests they always pass.
#[tokio::test]
async fn generate_and_get_runtime_logs() {
    let port = pick_unused_port().unwrap();
    let addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), port);
    const DEPLOYMENT_ID: &str = "runtime-fetch-logs-deployment-id";

    // Start the logger server in the background.
    tokio::task::spawn(async move {
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
    });

    // Ensure the logger server has time to start.
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // Start a subscriber and generate some logs.
    generate_runtime_logs(port, DEPLOYMENT_ID.into(), deploy);
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

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
            level: LogLevel::Trace,
            fields: json!({"message": "foo"}),
        },
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

    // Generate some logs with a fn not instrumented with deployment_id, and the
    // ID not added to the tracer attributes.
    generate_service_logs(port, DEPLOYMENT_ID.into(), deploy);

    let response = client
        .get_logs(Request::new(LogsRequest {
            deployment_id: DEPLOYMENT_ID.into(),
        }))
        .await
        .unwrap()
        .into_inner();

    // Check that no more logs have been recorded.
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
async fn generate_and_get_service_logs() {
    let port = pick_unused_port().unwrap();
    let addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), port);
    const DEPLOYMENT_ID: &str = "service-fetch-logs-deployment-id";

    // Start the logger server in the background.
    tokio::task::spawn(async move {
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
    });

    // Ensure the logger server has time to start.
    // TODO: find out why setting this lower causes spurious failures of these tests.
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // Start a subscriber and generate some logs using an instrumented deploy function.
    generate_service_logs(port, DEPLOYMENT_ID.into(), deploy_instrumented);
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

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

    // Generate some logs with a fn not instrumented with deployment_id.
    generate_service_logs(port, DEPLOYMENT_ID.into(), deploy);

    let response = client
        .get_logs(Request::new(LogsRequest {
            deployment_id: DEPLOYMENT_ID.into(),
        }))
        .await
        .unwrap()
        .into_inner();

    // Check that no more logs have been recorded.
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
async fn generate_and_stream_logs() {
    let port = pick_unused_port().unwrap();
    let addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), port);
    const DEPLOYMENT_ID: &str = "stream-logs-deployment-id";

    // Start the logger server in the background.
    tokio::spawn(async move {
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
    });

    // Ensure the server has started.
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;

    // Start a subscriber and generate some logs.
    generate_runtime_logs(port, DEPLOYMENT_ID.into(), foo);

    // Connect to the logger server so we can fetch logs.
    let dst = format!("http://localhost:{port}");
    let mut client = LoggerClient::connect(dst).await.unwrap();

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

    // Start a subscriber and generate some more logs.
    generate_runtime_logs(port, DEPLOYMENT_ID.into(), bar);

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

/// For the service logs the deployment id will be retrieved from the spans of functions
/// instrumented with the deployment_id field, this way we can choose which spans we want
/// to associate with a deployment and record in the logger.
fn generate_service_logs(port: u16, deployment_id: String, generator: fn(String)) {
    generate_logs(
        port,
        deployment_id,
        generator,
        vec![KeyValue::new("service.name", "test")],
    );
}

/// For the shuttle-runtime logs we want to add the deployment id to the top level attributes,
/// this way we can associate any logs coming from a runtime with a deployment.
fn generate_runtime_logs(port: u16, deployment_id: String, generator: fn(String)) {
    generate_logs(
        port,
        deployment_id.clone(),
        generator,
        vec![
            KeyValue::new("service.name", "test"),
            KeyValue::new("deployment_id", deployment_id),
        ],
    );
}

/// Helper function to setup a tracing subscriber and run an instrumented fn to produce logs.
fn generate_logs(
    port: u16,
    deployment_id: String,
    generator: fn(String),
    resources: Vec<KeyValue>,
) {
    // Set up tracing subscriber connected to the logger server.
    let tracer = opentelemetry_otlp::new_pipeline()
        .tracing()
        .with_exporter(
            opentelemetry_otlp::new_exporter()
                .tonic()
                .with_endpoint(format!("http://127.0.0.1:{port}")),
        )
        .with_trace_config(
            opentelemetry::sdk::trace::config()
                .with_resource(opentelemetry::sdk::Resource::new(resources)),
        )
        .install_simple()
        .unwrap();
    let otel_layer = tracing_opentelemetry::layer().with_tracer(tracer);

    let _guard = tracing_subscriber::registry()
        .with(otel_layer)
        .set_default();

    // Generate some logs.
    generator(deployment_id);
}

// deployment_id attribute not set.
#[instrument]
fn deploy(deployment_id: String) {
    error!("error");
    warn!("warn");
    info!(%deployment_id, "info");
    debug!("debug");
    trace!("trace");
    // This tests that we handle nested spans.
    foo(deployment_id);
}

#[instrument(fields(%deployment_id))]
fn deploy_instrumented(deployment_id: String) {
    error!("error");
    warn!("warn");
    info!(%deployment_id, "info");
    debug!("debug");
    trace!("trace");
}

#[instrument]
fn foo(deployment_id: String) {
    trace!("foo");
}

#[instrument]
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
