use std::net::SocketAddr;

use async_trait::async_trait;
use opentelemetry_proto::tonic::collector::trace::v1::{
    trace_service_server::{TraceService, TraceServiceServer},
    ExportTraceServiceRequest, ExportTraceServiceResponse,
};
use tokio::task::JoinHandle;
use tonic::{
    transport::{self, Server},
    Request, Response,
};

/// This is a simple logger server so the local runner has a logger server to connect to.
/// The tracing::fmt layer will handle writing logs to stdout for local runs.
pub struct LocalLogger;

impl LocalLogger {
    pub fn new() -> Self {
        Self
    }

    pub fn start(self, address: SocketAddr) -> JoinHandle<Result<(), transport::Error>> {
        tokio::spawn(async move {
            Server::builder()
                .add_service(TraceServiceServer::new(self))
                .serve(address)
                .await
        })
    }
}

#[async_trait]
impl TraceService for LocalLogger {
    async fn export(
        &self,
        _request: Request<ExportTraceServiceRequest>,
    ) -> std::result::Result<tonic::Response<ExportTraceServiceResponse>, tonic::Status> {
        Ok(Response::new(Default::default()))
    }
}
