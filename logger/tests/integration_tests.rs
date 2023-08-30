use std::time::{Duration, SystemTime};

use portpicker::pick_unused_port;
use pretty_assertions::assert_eq;
use rand::{distributions::Alphanumeric, thread_rng, Rng};
use shuttle_proto::logger::{
    logger_client::LoggerClient, LogItem, LogLine, LogsRequest, StoreLogsRequest,
};
use sqlx::__rt::timeout;
use tonic::Request;

mod local_postgres;
use local_postgres::LocalPostgresWrapper;

// static HOST_PORT: Lazy<Option<u16>> = Lazy::new(pick_unused_port);
// static POSTGRES_WRAPPER: Lazy<LocalPostgresWrapper> = Lazy::new(LocalPostgresWrapper::default);
use prost_types::Timestamp;

const SHUTTLE_SERVICE: &str = "test";

#[tokio::test]
async fn store_and_get_logs() {
    let postgres_wrapper = LocalPostgresWrapper::default();
    let logger_port = pick_unused_port().unwrap();
    let db_name: String = thread_rng()
        .sample_iter(&Alphanumeric)
        .take(10)
        .map(char::from)
        .collect();
    let deployment_id = "runtime-fetch-logs-deployment-id";

    let test_future = async move {
        let dst = format!("http://localhost:{logger_port}");
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
    };

    postgres_wrapper
        .run_against_underlying_container(test_future, logger_port, &db_name)
        .await;
}

#[tokio::test]
async fn get_stream_logs() {
    let postgres_wrapper = LocalPostgresWrapper::default();
    let logger_port = pick_unused_port().unwrap();
    let db_name: String = thread_rng()
        .sample_iter(&Alphanumeric)
        .take(10)
        .map(char::from)
        .collect();
    let deployment_id = "runtime-fetch-logs-deployment-id";

    let test_future = async move {
        let dst = format!("http://localhost:{logger_port}");
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

        let log = timeout(std::time::Duration::from_millis(500), response.message())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert_eq!(expected_stored_logs[1].clone().log_line.unwrap(), log);
    };

    postgres_wrapper
        .run_against_underlying_container(test_future, logger_port, &db_name)
        .await;
}
