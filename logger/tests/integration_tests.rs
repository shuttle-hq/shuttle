use std::net::{Ipv4Addr, SocketAddr};

use opentelemetry_proto::tonic::collector::logs::v1::logs_service_server::LogsServiceServer;
use portpicker::pick_unused_port;
use pretty_assertions::assert_eq;
use serde_json::{json, Value};
use shuttle_common::{
    backends::tracing::{DeploymentLayer, OtlpDeploymentLogRecorder},
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
use tracing::{debug, error, info, instrument, trace, warn};
use tracing_subscriber::prelude::*;
use uuid::Uuid;

#[tokio::test]
async fn logger() {
    let port = pick_unused_port().unwrap();
    let addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), port);

    let server_future = async {
        let sqlite = Sqlite::new_in_memory().await;

        Server::builder()
            .layer(JwtScopesLayer::new(vec![Scope::Logs]))
            .add_service(LogsServiceServer::new(ShuttleLogsOtlp::new(
                sqlite.get_sender(),
            )))
            .add_service(LoggerServer::new(Service::new(sqlite.get_sender(), sqlite)))
            .serve(addr)
            .await
            .unwrap()
    };

    let test_future = async {
        // Make sure the server starts first
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        let dst = format!("http://localhost:{port}");

        let subscriber = tracing_subscriber::registry().with(DeploymentLayer::new(
            OtlpDeploymentLogRecorder::new("test", &dst),
        ));
        let _guard = tracing::subscriber::set_default(subscriber);

        let mut client = LoggerClient::connect(dst).await.unwrap();

        let deployment_id = Uuid::new_v4();

        // Generate some logs
        deploy(deployment_id);

        tokio::time::sleep(std::time::Duration::from_millis(150)).await;

        // Get the generated logs
        let response = client
            .get_logs(Request::new(LogsRequest {
                deployment_id: deployment_id.to_string(),
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
    };

    select! {
        _ = server_future => panic!("server finished first"),
        _ = test_future => {},
    }
}

#[tokio::test]
async fn logger_stream() {
    let port = pick_unused_port().unwrap();
    let addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), port);

    let server_future = async {
        let sqlite = Sqlite::new_in_memory().await;

        Server::builder()
            .layer(JwtScopesLayer::new(vec![Scope::Logs]))
            .add_service(LogsServiceServer::new(ShuttleLogsOtlp::new(
                sqlite.get_sender(),
            )))
            .add_service(LoggerServer::new(Service::new(sqlite.get_sender(), sqlite)))
            .serve(addr)
            .await
            .unwrap()
    };

    let test_future = async {
        // Make sure the server starts first
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        let dst = format!("http://localhost:{port}");

        let subscriber = tracing_subscriber::registry().with(DeploymentLayer::new(
            OtlpDeploymentLogRecorder::new("test", &dst),
        ));
        let _guard = tracing::subscriber::set_default(subscriber);

        let mut client = LoggerClient::connect(dst).await.unwrap();

        let deployment_id = Uuid::new_v4();

        // Generate some logs
        foo(deployment_id);

        tokio::time::sleep(std::time::Duration::from_millis(150)).await;

        // Subscribe to stream
        let mut response = client
            .get_logs_stream(Request::new(LogsRequest {
                deployment_id: deployment_id.to_string(),
            }))
            .await
            .unwrap()
            .into_inner();

        let log = timeout(std::time::Duration::from_millis(1000), response.message())
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
        bar(deployment_id);

        tokio::time::sleep(std::time::Duration::from_millis(150)).await;

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
    };

    select! {
        _ = server_future => panic!("server finished first"),
        _ = test_future => {},
    }
}

#[instrument(fields(%deployment_id))]
fn deploy(deployment_id: Uuid) {
    error!("error");
    warn!("warn");
    info!(%deployment_id, "info");
    debug!("debug");
    trace!("trace");
}

#[instrument(fields(%deployment_id))]
fn foo(deployment_id: Uuid) {
    trace!("foo");
}

#[instrument(fields(%deployment_id))]
fn bar(deployment_id: Uuid) {
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
