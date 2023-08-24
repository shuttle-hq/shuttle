use std::{
    net::{Ipv4Addr, SocketAddr},
    time::{Duration, SystemTime},
};

use portpicker::pick_unused_port;
use pretty_assertions::assert_eq;
use prost_types::Timestamp;
use shuttle_common::claims::Scope;
use shuttle_common_tests::JwtScopesLayer;
use shuttle_logger::{Service, Sqlite};
use shuttle_proto::logger::{
    logger_client::LoggerClient, logger_server::LoggerServer, FetchedLogItem, LogsRequest,
    StoreLogsRequest, StoredLogItem,
};
use tokio::time::timeout;
use tonic::{transport::Server, Request};

const SHUTTLE_SERVICE: &str = "test";

// TODO: find out why these tests affect one-another. If running them together setting the timeouts
// low will cause them to fail spuriously. If running single tests they always pass.
#[tokio::test]
async fn store_and_get_logs() {
    let port = pick_unused_port().unwrap();
    let addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), port);
    const DEPLOYMENT_ID: &str = "runtime-fetch-logs-deployment-id";

    // Start the logger server in the background.
    let sqlite = Sqlite::new_in_memory().await;
    let sqlite_clone = sqlite.clone();
    let server = tokio::task::spawn(async move {
        Server::builder()
            .layer(JwtScopesLayer::new(vec![
                Scope::Logs,
                Scope::DeploymentPush,
            ]))
            .add_service(LoggerServer::new(Service::new(
                sqlite_clone.get_sender(),
                sqlite_clone,
            )))
            .serve(addr)
            .await
            .unwrap()
    });

    let test = tokio::task::spawn(async move {
        let dst = format!("http://localhost:{port}");
        let mut client = LoggerClient::connect(dst).await.unwrap();

        // Get the generated logs
        let expected_stored_logs = vec![
            StoredLogItem {
                deployment_id: DEPLOYMENT_ID.to_string(),
                service_name: SHUTTLE_SERVICE.to_string(),
                tx_timestamp: Some(Timestamp::from(SystemTime::UNIX_EPOCH)),
                data: "log 1 example".as_bytes().to_vec(),
            },
            StoredLogItem {
                deployment_id: DEPLOYMENT_ID.to_string(),
                service_name: SHUTTLE_SERVICE.to_string(),
                tx_timestamp: Some(Timestamp::from(
                    SystemTime::UNIX_EPOCH
                        .checked_add(Duration::from_secs(10))
                        .unwrap(),
                )),
                data: "log 2 example".as_bytes().to_vec(),
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
            }))
            .await
            .unwrap()
            .into_inner()
            .log_items;
        assert_eq!(
            logs,
            expected_stored_logs
                .into_iter()
                .map(Into::into)
                .collect::<Vec<FetchedLogItem>>()
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
    let addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), port);
    const DEPLOYMENT_ID: &str = "runtime-fetch-logs-deployment-id";

    // Start the logger server in the background.
    let sqlite = Sqlite::new_in_memory().await;
    let sqlite_clone = sqlite.clone();
    let server = tokio::task::spawn(async move {
        Server::builder()
            .layer(JwtScopesLayer::new(vec![
                Scope::Logs,
                Scope::DeploymentPush,
            ]))
            .add_service(LoggerServer::new(Service::new(
                sqlite_clone.get_sender(),
                sqlite_clone,
            )))
            .serve(addr)
            .await
            .unwrap()
    });

    let test = tokio::task::spawn(async move {
        let dst = format!("http://localhost:{port}");
        let mut client = LoggerClient::connect(dst).await.unwrap();

        // Get the generated logs
        let expected_stored_logs = vec![
            StoredLogItem {
                deployment_id: DEPLOYMENT_ID.to_string(),
                service_name: SHUTTLE_SERVICE.to_string(),
                tx_timestamp: Some(Timestamp::from(SystemTime::UNIX_EPOCH)),
                data: "log 1 example".as_bytes().to_vec(),
            },
            StoredLogItem {
                deployment_id: DEPLOYMENT_ID.to_string(),
                service_name: SHUTTLE_SERVICE.to_string(),
                tx_timestamp: Some(Timestamp::from(
                    SystemTime::UNIX_EPOCH
                        .checked_add(Duration::from_secs(10))
                        .unwrap(),
                )),
                data: "log 2 example".as_bytes().to_vec(),
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
            }))
            .await
            .unwrap()
            .into_inner();

        let log = timeout(std::time::Duration::from_millis(500), response.message())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert_eq!(FetchedLogItem::from(expected_stored_logs[0].clone()), log);

        let log = timeout(std::time::Duration::from_millis(500), response.message())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert_eq!(FetchedLogItem::from(expected_stored_logs[1].clone()), log);
    });

    tokio::select! {
        _ = server => panic!("server stopped first"),
        _ = test => ()
    }
}
