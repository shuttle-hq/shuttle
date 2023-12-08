use std::{
    net::{Ipv4Addr, SocketAddr},
    time::{Duration, SystemTime},
};

use ctor::dtor;
use once_cell::sync::Lazy;
use portpicker::pick_unused_port;
use shuttle_common::claims::Scope;
use shuttle_common_tests::JwtScopesLayer;
use shuttle_logger::{Postgres, Service};
use shuttle_proto::logger::{
    logger_client::LoggerClient, logger_server::LoggerServer, LogItem, LogLine, LogsRequest,
    StoreLogsRequest,
};
use sqlx::__rt::timeout;
use tokio::task::JoinHandle;
use tonic::{
    transport::{Server, Uri},
    Request,
};

use shuttle_common_tests::postgres::DockerInstance;

use prost_types::Timestamp;

const SHUTTLE_SERVICE: &str = "test";

static PG: Lazy<DockerInstance> = Lazy::new(DockerInstance::default);

#[dtor]
fn cleanup() {
    PG.cleanup();
}
mod needs_docker {
    use super::*;
    use pretty_assertions::assert_eq;

    #[tokio::test]
    async fn store_and_get_logs() {
        let logger_port = pick_unused_port().unwrap();
        let deployment_id = "runtime-fetch-logs-deployment-id";

        let server = spawn_server(logger_port);

        let test_future = tokio::spawn(async move {
            // Ensure the DB has been created and server has started.
            tokio::time::sleep(Duration::from_millis(300)).await;

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
        });

        tokio::select! {
            _ = server => panic!("server stopped first"),
            result = test_future => result.expect("test should succeed")
        }
    }

    #[tokio::test]
    async fn get_stream_logs() {
        let logger_port = pick_unused_port().unwrap();
        let deployment_id = "runtime-fetch-logs-deployment-id";

        let server = spawn_server(logger_port);
        let test_future = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(800)).await;

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
        });

        tokio::select! {
            _ = server => panic!("server stopped first"),
            result = test_future => result.expect("test should succeed")
        }
    }

    fn spawn_server(port: u16) -> JoinHandle<()> {
        let addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), port);
        let pg_uri = Uri::try_from(PG.get_unique_uri()).unwrap();

        tokio::task::spawn(async move {
            let pg = Postgres::new(&pg_uri).await;
            Server::builder()
                .layer(JwtScopesLayer::new(vec![Scope::Logs]))
                .add_service(LoggerServer::new(Service::new(pg.get_sender(), pg)))
                .serve(addr)
                .await
                .unwrap()
        })
    }
}
