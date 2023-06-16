use std::net::{Ipv4Addr, SocketAddr};

use portpicker::pick_unused_port;
use pretty_assertions::assert_eq;
use serde_json::{json, Value};
use shuttle_common::backends::tracing::{DeploymentLayer, OtlpDeploymentLogRecorder};
use shuttle_logger::{dal::Sqlite, Service};
use shuttle_proto::logger::{
    logger_client::LoggerClient, logger_server::LoggerServer, LogItem, LogLevel, LogsRequest,
};
use tokio::select;
use tonic::{transport::Server, Request};
use tracing::{debug, error, info, trace, warn};
use tracing_subscriber::prelude::*;
use ulid::Ulid;

#[tokio::test]
async fn logger() {
    let port = pick_unused_port().unwrap();
    let addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), port);

    let server_future = async {
        Server::builder()
            // .layer(JwtScopesLayer::new(vec![Scope::Resources]))
            .add_service(LoggerServer::new(Service::new(
                Sqlite::new_in_memory().await,
            )))
            .serve(addr)
            .await
            .unwrap()
    };

    let test_future = async {
        // Make sure the server starts first
        tokio::time::sleep(std::time::Duration::from_millis(1)).await;

        let dst = format!("http://localhost:{port}");

        let subscriber = tracing_subscriber::registry().with(DeploymentLayer::new(
            OtlpDeploymentLogRecorder::new("test", &dst),
        ));
        let _guard = tracing::subscriber::set_default(subscriber);

        let mut client = LoggerClient::connect(dst).await.unwrap();

        let deployment_id = Ulid::new();

        // Generate some logs
        deploy(deployment_id);

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

fn deploy(deployment_id: Ulid) {
    error!("error");
    warn!("warn");
    info!(%deployment_id, "info");
    debug!("debug");
    trace!("trace");
}

#[derive(Debug, Eq, PartialEq)]
struct MinLogItem {
    level: LogLevel,
    fields: Value,
}

impl From<LogItem> for MinLogItem {
    fn from(log: LogItem) -> Self {
        assert_eq!(log.service_name, "test");

        Self {
            level: log.level(),
            fields: serde_json::from_slice(&log.fields).unwrap(),
        }
    }
}
