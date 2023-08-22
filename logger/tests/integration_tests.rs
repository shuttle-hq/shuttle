use std::net::{Ipv4Addr, SocketAddr};

use opentelemetry::KeyValue;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_proto::tonic::collector::trace::v1::trace_service_server::TraceServiceServer;
use portpicker::pick_unused_port;
use pretty_assertions::assert_eq;
use serde_json::{json, Value};
use serial_test::serial;
use shuttle_common::{
    claims::Scope,
    tracing::{FILEPATH_KEY, LINENO_KEY, MESSAGE_KEY, NAMESPACE_KEY, TARGET_KEY},
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

/// Spawn the server and wait for it.
async fn spawn_server() -> u16 {
    let port = pick_unused_port().unwrap();
    let addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), port);
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
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    port
}

#[tokio::test]
#[serial]
async fn generate_and_get_runtime_logs() {
    let deployment_id = "runtime-fetch-logs-deployment-id";
    let port = spawn_server().await;

    // Start a subscriber and generate some logs.
    generate_runtime_logs(port, deployment_id.into(), deploy);

    // Get the generated logs
    let dst = format!("http://localhost:{port}");
    let mut client = LoggerClient::connect(dst).await.unwrap();
    let response = client
        .get_logs(Request::new(LogsRequest {
            deployment_id: deployment_id.into(),
        }))
        .await
        .unwrap()
        .into_inner();

    let quoted_deployment_id = format!("\"{deployment_id}\"");
    let expected = vec![
        MinLogItem {
            level: LogLevel::Info,
            fields: json!({"message": "[span] deploy", "deployment_id": quoted_deployment_id }),
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
            fields: json!({"message": "info", "deployment_id": deployment_id.to_string()}),
        },
        MinLogItem {
            level: LogLevel::Debug,
            fields: json!({"message": "debug"}),
        },
        MinLogItem {
            level: LogLevel::Trace,
            fields: json!({"message": "trace"}),
        },
        MinLogItem {
            level: LogLevel::Info,
            fields: json!({"message": "[span] span_name1", "deployment_id": quoted_deployment_id }),
        },
        MinLogItem {
            level: LogLevel::Trace,
            fields: json!({"message": "inside span 1 event"}),
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
    generate_service_logs(port, deployment_id.into(), deploy);

    let response = client
        .get_logs(Request::new(LogsRequest {
            deployment_id: deployment_id.into(),
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
#[serial]
async fn generate_and_get_service_logs() {
    let deployment_id = "service-fetch-logs-deployment-id";
    let port = spawn_server().await;

    // Start a subscriber and generate some logs using an instrumented deploy function.
    generate_service_logs(port, deployment_id.into(), deploy_instrumented);

    // Get the generated logs
    let dst = format!("http://localhost:{port}");
    let mut client = LoggerClient::connect(dst).await.unwrap();
    let response = client
        .get_logs(Request::new(LogsRequest {
            deployment_id: deployment_id.into(),
        }))
        .await
        .unwrap()
        .into_inner();

    let expected = vec![
        MinLogItem {
            level: LogLevel::Info,
            fields: json!({"message": "[span] deploy_instrumented", "deployment_id": deployment_id.to_string() }),
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
            fields: json!({"message": "info", "deployment_id": deployment_id.to_string()}),
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
    generate_service_logs(port, deployment_id.into(), deploy);

    let response = client
        .get_logs(Request::new(LogsRequest {
            deployment_id: deployment_id.into(),
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
#[serial]
async fn generate_and_stream_logs() {
    let deployment_id = "stream-logs-deployment-id";
    let port = spawn_server().await;

    // Start a subscriber and generate some logs.
    generate_runtime_logs(port, deployment_id.into(), span_name1);

    // Connect to the logger server so we can fetch logs.
    let dst = format!("http://localhost:{port}");
    let mut client = LoggerClient::connect(dst).await.unwrap();

    // Subscribe to stream
    let mut response = client
        .get_logs_stream(Request::new(LogsRequest {
            deployment_id: deployment_id.into(),
        }))
        .await
        .unwrap()
        .into_inner();

    let log = timeout(std::time::Duration::from_millis(500), response.message())
        .await
        .unwrap()
        .unwrap()
        .unwrap();

    let quoted_deployment_id = format!("\"{deployment_id}\"");
    assert_eq!(
        MinLogItem::from(log),
        MinLogItem {
            level: LogLevel::Info,
            fields: json!({"message": "[span] span_name1", "deployment_id": quoted_deployment_id}),
        },
    );

    let log = timeout(std::time::Duration::from_millis(500), response.message())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(
        MinLogItem::from(log),
        MinLogItem {
            level: LogLevel::Trace,
            fields: json!({"message": "inside span 1 event"}),
        },
    );

    // Start a subscriber and generate some more logs.
    generate_runtime_logs(port, deployment_id.into(), span_name2);

    let log = timeout(std::time::Duration::from_millis(500), response.message())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(
        MinLogItem::from(log),
        MinLogItem {
            level: LogLevel::Trace,
            fields: json!({"message": "inside span 2 event"}),
        },
    );

    let log = timeout(std::time::Duration::from_millis(500), response.message())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(
        MinLogItem::from(log),
        MinLogItem {
            level: LogLevel::Info,
            fields: json!({"message": "[span] span_name2", "deployment_id": quoted_deployment_id}),
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
    span_name1(deployment_id);
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
fn span_name1(deployment_id: String) {
    trace!("inside span 1 event");
}

#[instrument]
fn span_name2(deployment_id: String) {
    trace!("inside span 2 event");
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

            let message = map.get(MESSAGE_KEY).unwrap();
            // Span logs don't contain a target field
            if !message.as_str().unwrap().starts_with("[span] ") {
                let target = map.remove(TARGET_KEY).unwrap();
                assert_eq!(target, "integration_tests");
            } else {
                // We want to remove what's not of interest for checking
                // the spans are containing the right information.
                let _ = map.remove("busy_ns").unwrap();
                let _ = map.remove("idle_ns").unwrap();
                let _ = map.remove("thread.id").unwrap();
                let _ = map.remove("thread.name").unwrap();
            }

            let filepath = map.remove(FILEPATH_KEY).unwrap();
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
