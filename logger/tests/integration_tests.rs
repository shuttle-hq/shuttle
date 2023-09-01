use std::{
    net::{Ipv4Addr, SocketAddr},
    str,
    time::{Duration, SystemTime},
};

use portpicker::pick_unused_port;
use pretty_assertions::assert_eq;
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
const DEPLOYMENT_ID: &str = "runtime-fetch-logs-deployment-id";

#[tokio::test]
async fn store_and_get_logs() {
    let port = pick_unused_port().unwrap();

    let server = get_server(port);
    tokio::time::sleep(Duration::from_secs(1)).await;
    let test = tokio::task::spawn(async move {
        let dst = format!("http://localhost:{port}");
        let mut client = LoggerClient::connect(dst).await.unwrap();

        // Get the generated logs
        let expected_stored_logs = vec![
            LogItem {
                deployment_id: DEPLOYMENT_ID.to_string(),
                log_line: Some(LogLine {
                    service_name: SHUTTLE_SERVICE.to_string(),
                    tx_timestamp: Some(Timestamp::from(SystemTime::UNIX_EPOCH)),
                    data: "log 1 example".as_bytes().to_vec(),
                }),
            },
            LogItem {
                deployment_id: DEPLOYMENT_ID.to_string(),
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
                deployment_id: DEPLOYMENT_ID.into(),
                ..Default::default()
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

    // Start the logger server in the background.
    let server = get_server(port);
    tokio::time::sleep(Duration::from_secs(1)).await;
    let test = tokio::task::spawn(async move {
        let dst = format!("http://localhost:{port}");
        let mut client = LoggerClient::connect(dst).await.unwrap();

        // Get the generated logs
        let expected_stored_logs = vec![
            LogItem {
                deployment_id: DEPLOYMENT_ID.to_string(),
                log_line: Some(LogLine {
                    service_name: SHUTTLE_SERVICE.to_string(),
                    tx_timestamp: Some(Timestamp::from(SystemTime::UNIX_EPOCH)),
                    data: "log 1 example".as_bytes().to_vec(),
                }),
            },
            LogItem {
                deployment_id: DEPLOYMENT_ID.to_string(),
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
                deployment_id: DEPLOYMENT_ID.into(),
                ..Default::default()
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
    });

    tokio::select! {
        _ = server => panic!("server stopped first"),
        _ = test => ()
    }
}

#[tokio::test]
async fn paginate_logs() {
    let port = pick_unused_port().unwrap();

    // Start the logger server in the background.
    let server = get_server(port);
    tokio::time::sleep(Duration::from_secs(1)).await;
    let test = tokio::task::spawn(async move {
        let dst = format!("http://localhost:{port}");
        let mut client = LoggerClient::connect(dst).await.unwrap();

        let mut test_logs = Vec::new();
        for i in 1..=50 {
            test_logs.push(LogItem {
                deployment_id: DEPLOYMENT_ID.to_string(),
                log_line: Some(LogLine {
                    service_name: SHUTTLE_SERVICE.to_string(),
                    tx_timestamp: Some(Timestamp::from(SystemTime::UNIX_EPOCH)),
                    data: format!("log {i} example").as_bytes().to_vec(),
                }),
            });
        }

        let response = client
            .store_logs(Request::new(StoreLogsRequest {
                logs: test_logs.clone(),
            }))
            .await
            .unwrap()
            .into_inner();
        assert!(response.success);

        // Get logs
        let logs = client
            .get_logs(Request::new(LogsRequest {
                deployment_id: DEPLOYMENT_ID.into(),
                page: Some(1),
                limit: Some(25),
            }))
            .await
            .unwrap()
            .into_inner()
            .log_items;

        assert_eq!(25, logs.len());
        assert_eq!(
            "log 26 example",
            str::from_utf8(&logs.first().unwrap().data).unwrap()
        );

        let logs = client
            .get_logs(Request::new(LogsRequest {
                deployment_id: DEPLOYMENT_ID.into(),
                page: Some(2),
                limit: Some(20),
            }))
            .await
            .unwrap()
            .into_inner()
            .log_items;

        assert_eq!(10, logs.len());
        assert_eq!(
            "log 41 example",
            str::from_utf8(&logs.first().unwrap().data).unwrap()
        );
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
