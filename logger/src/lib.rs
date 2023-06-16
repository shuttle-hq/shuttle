use async_trait::async_trait;
use dal::Dal;
use opentelemetry_proto::tonic::collector::logs::v1::{
    logs_service_server::LogsService, ExportLogsServiceRequest, ExportLogsServiceResponse,
};
use shuttle_proto::logger::{logger_server::Logger, LogItem, LogsRequest, LogsResponse};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status};

pub mod dal;

pub struct ShuttleLogsOtlp;

impl ShuttleLogsOtlp {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl LogsService for ShuttleLogsOtlp {
    async fn export(
        &self,
        request: Request<ExportLogsServiceRequest>,
    ) -> Result<Response<ExportLogsServiceResponse>, Status> {
        let request = request.into_inner();

        println!("{request:#?}");

        Ok(Response::new(ExportLogsServiceResponse {
            partial_success: None,
        }))
    }
}

pub struct Service<D> {
    dal: D,
}

impl<D> Service<D>
where
    D: Dal + Send + Sync + 'static,
{
    pub fn new(dal: D) -> Self {
        Self { dal }
    }
}

#[async_trait]
impl<D> Logger for Service<D>
where
    D: Dal + Send + Sync + 'static,
{
    async fn get_logs(
        &self,
        _request: Request<LogsRequest>,
    ) -> Result<Response<LogsResponse>, Status> {
        let result = LogsResponse {
            log_items: Vec::new(),
        };

        Ok(Response::new(result))
    }

    type GetLogsStreamStream = ReceiverStream<Result<LogItem, Status>>;

    async fn get_logs_stream(
        &self,
        _request: Request<LogsRequest>,
    ) -> Result<Response<Self::GetLogsStreamStream>, Status> {
        let (_tx, rx) = mpsc::channel(1);

        Ok(Response::new(ReceiverStream::new(rx)))
    }
}
