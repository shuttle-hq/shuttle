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

mod helpers;
use helpers::{exec_psql, DockerInstance};

use prost_types::Timestamp;
use uuid::Uuid;

const SHUTTLE_SERVICE: &str = "test";

static PG: Lazy<DockerInstance> = Lazy::new(DockerInstance::default);

#[dtor]
fn cleanup() {
    PG.cleanup();
}
mod needs_docker {
    use super::*;
    use axum::{error_handling::HandleErrorLayer, BoxError};
    use futures::future::join_all;
    use pretty_assertions::assert_eq;
    use shuttle_logger::rate_limiting::{tonic_error, TonicPeerIpKeyExtractor};
    use tower_governor::{governor::GovernorConfigBuilder, GovernorLayer};

    #[tokio::test]
    async fn store_and_get_logs() {
        let logger_port = pick_unused_port().unwrap();
        let deployment_id = "runtime-fetch-logs-deployment-id";

        // Create a unique database name so we have a new database for each test.
        let db_name = Uuid::new_v4().to_string();

        let server = spawn_server(logger_port, db_name);

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

        // Create a unique database name so we have a new database for each test.
        let db_name = Uuid::new_v4().to_string();

        let server = spawn_server(logger_port, db_name);

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

    #[tokio::test]
    async fn store_and_get_logs_rate_limited() {
        let logger_port = pick_unused_port().unwrap();
        let deployment_id = "runtime-fetch-logs-deployment-id";

        // Create a unique database name so we have a new database for each test.
        let db_name = Uuid::new_v4().to_string();

        let server = spawn_server(logger_port, db_name);

        let test_future = tokio::spawn(async move {
            // Ensure the DB has been created and server has started.
            tokio::time::sleep(Duration::from_millis(300)).await;

            let dst = format!("http://localhost:{logger_port}");
            let mut client = LoggerClient::connect(dst).await.unwrap();

            let store_logs = || async {
                client
                    .clone()
                    .store_logs(Request::new(StoreLogsRequest {
                        logs: vec![LogItem {
                            deployment_id: deployment_id.to_string(),
                            log_line: Some(LogLine {
                                service_name: SHUTTLE_SERVICE.to_string(),
                                tx_timestamp: Some(Timestamp::from(SystemTime::UNIX_EPOCH)),
                                data: ("log example").as_bytes().to_vec(),
                            }),
                        }],
                    }))
                    .await
            };

            // Two concurrent requests succeeds.
            let futures = (0..2).map(|_| store_logs());
            let result = join_all(futures).await;

            assert!(result.iter().all(|response| response.is_ok()));

            // Allow rate limiter time to regenerate.
            tokio::time::sleep(Duration::from_millis(1000)).await;

            // If we send 6 concurrent requests, 5 will succeed.
            let futures = (0..6).map(|_| store_logs());
            let result = join_all(futures).await;

            assert_eq!(result.iter().filter(|response| response.is_ok()).count(), 5);

            // Check that the error has the expected status and rate limiting headers.
            result
                .iter()
                .filter(|response| response.is_err())
                .for_each(|err| {
                    let err = err.as_ref().unwrap_err();

                    assert_eq!(err.code(), tonic::Code::Unavailable);
                    assert!(err.message().contains("too many requests"));

                    let expected = [
                        "x-ratelimit-remaining",
                        "x-ratelimit-after",
                        "x-ratelimit-limit",
                    ];

                    let headers = err.metadata();
                    assert!(expected.into_iter().all(|key| headers.contains_key(key)));
                });

            // Allow rate limiter to regenerate.
            tokio::time::sleep(Duration::from_millis(1000)).await;

            // Verify that all the logs that weren't rate limited were persisted in the logger.
            let logs = client
                .get_logs(Request::new(LogsRequest {
                    deployment_id: deployment_id.into(),
                }))
                .await
                .unwrap()
                .into_inner()
                .log_items;

            assert_eq!(logs.len(), 7);
        });

        tokio::select! {
            _ = server => panic!("server stopped first"),
            result = test_future => result.expect("test should succeed")
        }
    }

    fn spawn_server(port: u16, db_name: String) -> JoinHandle<()> {
        let addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), port);

        // Get the PG uri first so the static PG is initialized.
        let pg_uri = Uri::try_from(format!("{}/{}", PG.uri, db_name)).unwrap();

        exec_psql(&format!(r#"CREATE DATABASE "{}";"#, &db_name));

        let governor_config = GovernorConfigBuilder::default()
            .per_second(1)
            .burst_size(6)
            .use_headers()
            .key_extractor(TonicPeerIpKeyExtractor)
            .finish()
            .unwrap();

        tokio::task::spawn(async move {
            let pg = Postgres::new(&pg_uri).await;
            Server::builder()
                .layer(JwtScopesLayer::new(vec![Scope::Logs]))
                .layer(HandleErrorLayer::new(|e: BoxError| async move {
                    tonic_error(e)
                }))
                .layer(GovernorLayer {
                    config: &governor_config,
                })
                .add_service(LoggerServer::new(Service::new(pg.get_sender(), pg)))
                .serve(addr)
                .await
                .unwrap()
        })
    }
}
