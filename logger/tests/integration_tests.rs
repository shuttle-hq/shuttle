use std::{
    net::{Ipv4Addr, SocketAddr},
    time::{Duration, SystemTime},
};

use portpicker::pick_unused_port;
use pretty_assertions::assert_eq;
use rand::{distributions::Alphanumeric, thread_rng, Rng};
use serde_json::{json, Value};
use shuttle_common::{
    backends::tracing::{DeploymentLayer, OtlpDeploymentLogRecorder},
    tracing::{FILEPATH_KEY, LINENO_KEY, NAMESPACE_KEY, TARGET_KEY},
};
use shuttle_proto::logger::{logger_client::LoggerClient, LogItem, LogLevel, LogsRequest};
use sqlx::__rt::timeout;
use tonic::Request;
use tracing::{debug, error, info, instrument, trace, warn};
use tracing_subscriber::prelude::__tracing_subscriber_SubscriberExt;

mod local_postgres;
use local_postgres::LocalPostgresWrapper;

// static HOST_PORT: Lazy<Option<u16>> = Lazy::new(pick_unused_port);
// static POSTGRES_WRAPPER: Lazy<LocalPostgresWrapper> = Lazy::new(LocalPostgresWrapper::default);
use prost_types::Timestamp;
use shuttle_common::claims::Scope;
use shuttle_common_tests::JwtScopesLayer;
use shuttle_logger::{Service, Sqlite};
use shuttle_proto::logger::{
    logger_client::LoggerClient, logger_server::LoggerServer, LogItem, LogLine, LogsRequest,
    StoreLogsRequest,
};
use tokio::{task::JoinHandle, time::timeout};
use tonic::{transport::Server, Request};

const SHUTTLE_SERVICE: &str = "test";

#[tokio::test]
async fn store_and_get_logs() {
    let port = pick_unused_port().unwrap();
    let deployment_id = "runtime-fetch-logs-deployment-id";

    let server = get_server(port);
    let test = tokio::task::spawn(async move {
        let dst = format!("http://localhost:{port}");
        let mut client = LoggerClient::connect(dst).await.unwrap();

        // Get the generated logs
        let expected_stored_logs = vec![
            LogItem {
                deployment_id: deployment_id.to_string(),
                log_line: Some(LogLine {
                    service_name: SHUTTLE_SERVICE.to_string(),
                    tx_timestamp: Some(Timestamp::from(SystemTime::UNIX_EPOCH)),
                    data: "log 1 example".as_bytes().to_vec(),
                }),
            },
            LogItem {
                deployment_id: deployment_id.to_string(),
                log_line: Some(LogLine {
                    service_name: SHUTTLE_SERVICE.to_string(),
                    tx_timestamp: Some(Timestamp::from(
                        SystemTime::UNIX_EPOCH
                            .checked_add(Duration::from_secs(10))
                            .unwrap(),
                    )),
                    data: "log 2 example".as_bytes().to_vec(),
                }),
            },
        ];
        let response = client
            .store_logs(Request::new(StoreLogsRequest {
                logs: expected_stored_logs.clone(),
            }))
            .await
            .unwrap()
            .into_inner();
        assert!(response.success);

        // Get logs
        let logs = client
            .get_logs(Request::new(LogsRequest {
                deployment_id: deployment_id.into(),
            }))
            .await
            .unwrap()
            .into_inner()
            .log_items;
        assert_eq!(
            logs,
            expected_stored_logs
                .into_iter()
                .map(|log| log.log_line.unwrap())
                .collect::<Vec<LogLine>>()
        );
    });

    tokio::select! {
        _ = server => panic!("server stopped first"),
        _ = test => ()
    }
}

#[tokio::test]
async fn get_stream_logs() {
    let port = pick_unused_port().unwrap();
    let deployment_id = "runtime-fetch-logs-deployment-id";

    // Start the logger server in the background.
    let server = get_server(port);
    let test = tokio::task::spawn(async move {
        let dst = format!("http://localhost:{port}");
        let mut client = LoggerClient::connect(dst).await.unwrap();

        // Get the generated logs
        let expected_stored_logs = vec![
            LogItem {
                deployment_id: deployment_id.to_string(),
                log_line: Some(LogLine {
                    service_name: SHUTTLE_SERVICE.to_string(),
                    tx_timestamp: Some(Timestamp::from(SystemTime::UNIX_EPOCH)),
                    data: "log 1 example".as_bytes().to_vec(),
                }),
            },
            LogItem {
                deployment_id: deployment_id.to_string(),
                log_line: Some(LogLine {
                    service_name: SHUTTLE_SERVICE.to_string(),
                    tx_timestamp: Some(Timestamp::from(
                        SystemTime::UNIX_EPOCH
                            .checked_add(Duration::from_secs(10))
                            .unwrap(),
                    )),
                    data: "log 2 example".as_bytes().to_vec(),
                }),
            },
        ];

        let response = client
            .store_logs(Request::new(StoreLogsRequest {
                logs: expected_stored_logs.clone(),
            }))
            .await
            .unwrap()
            .into_inner();
        assert!(response.success);

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
        assert_eq!(expected_stored_logs[0].clone().log_line.unwrap(), log);

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
        assert_eq!(expected_stored_logs[1].clone().log_line.unwrap(), log);
    });

    tokio::select! {
        _ = server => panic!("server stopped first"),
        _ = test => ()
    }
}

fn get_server(port: u16) -> JoinHandle<()> {
    let addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), port);
    tokio::task::spawn(async move {
        let sqlite = Sqlite::new_in_memory().await;
        Server::builder()
            .layer(JwtScopesLayer::new(vec![Scope::Logs]))
            .add_service(LoggerServer::new(Service::new(sqlite.get_sender(), sqlite)))
            .serve(addr)
            .await
            .unwrap()
    })
}
