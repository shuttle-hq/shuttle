use std::{
    net::{Ipv4Addr, SocketAddr},
    time::Duration,
};

use portpicker::pick_unused_port;
use shuttle_proto::logger::{
    self,
    logger_server::{Logger, LoggerServer},
    LogLine, LogsRequest, LogsResponse, StoreLogsRequest, StoreLogsResponse,
};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{async_trait, transport::Server, Request, Response, Status};

pub struct MockedLogger;

#[async_trait]
impl Logger for MockedLogger {
    async fn store_logs(
        &self,
        _: Request<StoreLogsRequest>,
    ) -> Result<Response<StoreLogsResponse>, Status> {
        Ok(Response::new(StoreLogsResponse { success: true }))
    }

    async fn get_logs(&self, _: Request<LogsRequest>) -> Result<Response<LogsResponse>, Status> {
        Ok(Response::new(LogsResponse {
            log_items: Vec::new(),
        }))
    }

    type GetLogsStreamStream = ReceiverStream<Result<LogLine, Status>>;

    async fn get_logs_stream(
        &self,
        _: Request<LogsRequest>,
    ) -> Result<Response<Self::GetLogsStreamStream>, Status> {
        let (_, rx) = mpsc::channel(1);
        Ok(Response::new(ReceiverStream::new(rx)))
    }
}

pub async fn get_mocked_logger_client(logger: impl Logger) -> logger::Client {
    let logger_addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), pick_unused_port().unwrap());
    let logger_uri = format!("http://{}", logger_addr);
    tokio::spawn(async move {
        Server::builder()
            .add_service(LoggerServer::new(logger))
            .serve(logger_addr)
            .await
    });

    // Wait for the logger server to start before creating a client.
    tokio::time::sleep(Duration::from_millis(200)).await;

    logger::get_client(logger_uri.parse().unwrap()).await
}
