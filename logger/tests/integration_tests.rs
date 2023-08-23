use std::{
    io::{self},
    net::{Ipv4Addr, SocketAddr},
    sync::Mutex,
    time::SystemTime,
};

use chrono::Utc;
use portpicker::pick_unused_port;
use pretty_assertions::assert_eq;
use prost_types::Timestamp;
use shuttle_common::claims::Scope;
use shuttle_common_tests::JwtScopesLayer;
use shuttle_logger::{Service, Sqlite};
use shuttle_proto::logger::{
    logger_client::LoggerClient, logger_server::LoggerServer, LogItem, LogsRequest,
    StoreLogsRequest,
};
use tokio::{
    sync::mpsc::{UnboundedReceiver, UnboundedSender},
    task::JoinHandle,
    time::timeout,
};
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
            .layer(JwtScopesLayer::new(vec![
                Scope::Logs,
                Scope::DeploymentPush,
            ]))
            .add_service(LoggerServer::new(Service::new(sqlite.get_sender(), sqlite)))
            .serve(addr)
            .await
            .unwrap()
    });

    // Ensure the logger server has time to start.
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let dst = format!("http://localhost:{port}");
    let mut client = LoggerClient::connect(dst).await.unwrap();

    // Start a subscriber and generate some logs.
    generate_runtime_logs(client.clone(), DEPLOYMENT_ID.into(), deploy);
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

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
            data: "ERROR deploy{deployment_id=\"runtime-fetch-logs-deployment-id\"}: integration_tests: error\n".to_string(),
        },
        MinLogItem {
            data: " WARN deploy{deployment_id=\"runtime-fetch-logs-deployment-id\"}: integration_tests: warn\n".to_string(),
        },
        MinLogItem {
            data: format!(" INFO deploy{{deployment_id=\"runtime-fetch-logs-deployment-id\"}}: integration_tests: info deployment_id={DEPLOYMENT_ID}\n"),
        },
        MinLogItem {
            data: "DEBUG deploy{deployment_id=\"runtime-fetch-logs-deployment-id\"}: integration_tests: debug\n".to_string(),
        },
        MinLogItem {
            data: "TRACE deploy{deployment_id=\"runtime-fetch-logs-deployment-id\"}: integration_tests: trace\n".to_string(),
        },
        MinLogItem {
            data: format!("TRACE deploy{{deployment_id=\"runtime-fetch-logs-deployment-id\"}}:span_name1{{deployment_id=\"{DEPLOYMENT_ID}\"}}: integration_tests: inside span 1 event\n"),
        }
    ];

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
            .layer(JwtScopesLayer::new(vec![
                Scope::DeploymentPush,
                Scope::Logs,
            ]))
            .add_service(LoggerServer::new(Service::new(sqlite.get_sender(), sqlite)))
            .serve(addr)
            .await
            .unwrap()
    });

    // Ensure the logger server has time to start.
    // TODO: find out why setting this lower causes spurious failures of these tests.
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let dst = format!("http://localhost:{port}");
    let mut client = LoggerClient::connect(dst).await.unwrap();

    // Start a subscriber and generate some logs using an instrumented deploy function.
    generate_service_logs(client.clone(), DEPLOYMENT_ID.into(), deploy_instrumented);
    tokio::time::sleep(std::time::Duration::from_millis(1000)).await;

    // Get the generated logs
    let response = client
        .get_logs(Request::new(LogsRequest {
            deployment_id: DEPLOYMENT_ID.into(),
        }))
        .await
        .unwrap()
        .into_inner();

    let mut expected = vec![
        MinLogItem {
            data: format!("ERROR deploy_instrumented{{deployment_id={DEPLOYMENT_ID}}}: integration_tests: error\n"),
        },
        MinLogItem {
            data: format!(" WARN deploy_instrumented{{deployment_id={DEPLOYMENT_ID}}}: integration_tests: warn\n"),
        },
        MinLogItem {
            data: format!(" INFO deploy_instrumented{{deployment_id={DEPLOYMENT_ID}}}: integration_tests: info deployment_id={DEPLOYMENT_ID}\n"),
        },
        MinLogItem {
            data: format!("DEBUG deploy_instrumented{{deployment_id={DEPLOYMENT_ID}}}: integration_tests: debug\n"),
        },
        MinLogItem {
            data: format!("TRACE deploy_instrumented{{deployment_id={DEPLOYMENT_ID}}}: integration_tests: trace\n"),
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
    generate_service_logs(client.clone(), DEPLOYMENT_ID.into(), deploy);
    tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
    let response = client
        .get_logs(Request::new(LogsRequest {
            deployment_id: DEPLOYMENT_ID.into(),
        }))
        .await
        .unwrap()
        .into_inner();

    // Check that more logs were added.
    expected.extend(vec![
        MinLogItem {
            data: format!("ERROR deploy{{deployment_id=\"{DEPLOYMENT_ID}\"}}: integration_tests: error\n"),
        },
        MinLogItem {
            data: format!(" WARN deploy{{deployment_id=\"{DEPLOYMENT_ID}\"}}: integration_tests: warn\n"),
        },
        MinLogItem {
            data: format!(" INFO deploy{{deployment_id=\"{DEPLOYMENT_ID}\"}}: integration_tests: info deployment_id={DEPLOYMENT_ID}\n"),
        },
        MinLogItem {
            data: format!("DEBUG deploy{{deployment_id=\"{DEPLOYMENT_ID}\"}}: integration_tests: debug\n"),
        },
        MinLogItem {
            data: format!("TRACE deploy{{deployment_id=\"{DEPLOYMENT_ID}\"}}: integration_tests: trace\n"),
        },
        MinLogItem {
            data: format!("TRACE deploy{{deployment_id=\"{DEPLOYMENT_ID}\"}}:span_name1{{deployment_id=\"{DEPLOYMENT_ID}\"}}: integration_tests: inside span 1 event\n"),
        },
    ]);
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
            .layer(JwtScopesLayer::new(vec![
                Scope::DeploymentPush,
                Scope::Logs,
            ]))
            .add_service(LoggerServer::new(Service::new(sqlite.get_sender(), sqlite)))
            .serve(addr)
            .await
            .unwrap()
    });

    // Ensure the server has started.
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;

    // Connect to the logger server so we can fetch logs.
    let dst = format!("http://localhost:{port}");
    let mut client = LoggerClient::connect(dst).await.unwrap();

    // Start a subscriber and generate some logs.
    generate_runtime_logs(client.clone(), DEPLOYMENT_ID.into(), span_name1);

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
            data: format!("TRACE span_name1{{deployment_id=\"{DEPLOYMENT_ID}\"}}: integration_tests: inside span 1 event\n")
        },
    );

    // Start a subscriber and generate some more logs.
    generate_runtime_logs(client.clone(), DEPLOYMENT_ID.into(), span_name2);

    let log = timeout(std::time::Duration::from_millis(500), response.message())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(
        MinLogItem::from(log),
        MinLogItem {
            data: format!("TRACE span_name2{{deployment_id=\"{DEPLOYMENT_ID}\"}}: integration_tests: inside span 2 event\n")
        },
    );
}

/// For the service logs the deployment id will be retrieved from the spans of functions
/// instrumented with the deployment_id field, this way we can choose which spans we want
/// to associate with a deployment and record in the logger.
fn generate_service_logs(
    client: LoggerClient<tonic::transport::Channel>,
    deployment_id: String,
    generator: fn(String),
) {
    generate_tracing_logs(client, deployment_id, generator);
}

/// For the shuttle-runtime logs we want to add the deployment id to the top level attributes,
/// this way we can associate any logs coming from a runtime with a deployment.
fn generate_runtime_logs(
    client: LoggerClient<tonic::transport::Channel>,
    deployment_id: String,
    generator: fn(String),
) {
    generate_tracing_logs(client, deployment_id, generator);
}

/// Helper function to setup a tracing subscriber and run an instrumented fn to produce logs.
fn generate_tracing_logs(
    client: LoggerClient<tonic::transport::Channel>,
    deployment_id: String,
    generator: fn(String),
) {
    // Set up tracing subscriber connected to the logger server
    let layer = tracing_subscriber::fmt::layer()
        .with_writer(Mutex::new(LoggerLayer::new(
            deployment_id.clone(),
            SHUTTLE_SERVICE.into(),
            client,
        )))
        .with_ansi(false);
    let _guard = tracing_subscriber::registry().with(layer).set_default();

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

const SHUTTLE_SERVICE: &str = "test";

#[derive(Debug, Eq, PartialEq)]
struct MinLogItem {
    data: String,
}

impl From<LogItem> for MinLogItem {
    fn from(log: LogItem) -> Self {
        assert_eq!(log.service_name, SHUTTLE_SERVICE);

        let data = String::from_utf8(log.data)
            .expect("to have valid utf8 log data")
            .split_once(' ')
            .unwrap()
            .1
            .to_string();

        Self { data }
    }
}

struct LoggerLayer {
    deployment_id: String,
    shuttle_service: String,
    tx: UnboundedSender<Vec<LogItem>>,
    _logs_forwarding_task: JoinHandle<()>,
}

impl LoggerLayer {
    pub fn new(
        deployment_id: String,
        shuttle_service: String,
        mut client: LoggerClient<tonic::transport::Channel>,
    ) -> Self {
        let (tx, mut rx): (
            UnboundedSender<Vec<LogItem>>,
            UnboundedReceiver<Vec<LogItem>>,
        ) = tokio::sync::mpsc::unbounded_channel();
        let handle = tokio::task::spawn(async move {
            while let Some(logs) = rx.recv().await {
                // service_tx.broadcast(logs).await.expect("to not fail");
                // Get the generated logs
                let _ = client
                    .store_logs(Request::new(StoreLogsRequest { logs }))
                    .await
                    .unwrap()
                    .into_inner();
            }
        });
        Self {
            deployment_id,
            shuttle_service,
            tx,
            _logs_forwarding_task: handle,
        }
    }
}

impl io::Write for LoggerLayer {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if self
            .tx
            .send(vec![LogItem {
                deployment_id: self.deployment_id.clone(),
                service_name: self.shuttle_service.clone(),
                tx_timestamp: Some(Timestamp::from(SystemTime::from(Utc::now()))),
                data: buf.to_vec(),
            }])
            .is_ok()
        {
            return Ok(buf.len());
        }

        Ok(0)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
