use async_trait::async_trait;
use dal::{Dal, DalError, Log};
use opentelemetry_proto::tonic::collector::logs::v1::{
    logs_service_server::LogsService, ExportLogsServiceRequest, ExportLogsServiceResponse,
};
use shuttle_common::{backends::auth::VerifyClaim, claims::Scope};
use shuttle_proto::logger::{logger_server::Logger, LogItem, LogsRequest, LogsResponse};
use thiserror::Error;
use tokio::sync::{broadcast, mpsc};
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status};
use ulid::DecodeError;

pub mod dal;

/// A wrapper to capture any error possible with this service
#[derive(Error, Debug)]
pub enum Error {
    #[error("could not decode id: {0}")]
    UlidDecode(#[from] DecodeError),

    #[error("failed to interact with database: {0}")]
    Dal(#[from] DalError),
}

impl From<Error> for Status {
    fn from(error: Error) -> Self {
        Self::internal(error.to_string())
    }
}

pub struct ShuttleLogsOtlp {
    tx: broadcast::Sender<Vec<Log>>,
}

impl ShuttleLogsOtlp {
    pub fn new(tx: broadcast::Sender<Vec<Log>>) -> Self {
        Self { tx }
    }
}

#[async_trait]
impl LogsService for ShuttleLogsOtlp {
    async fn export(
        &self,
        request: Request<ExportLogsServiceRequest>,
    ) -> Result<Response<ExportLogsServiceResponse>, Status> {
        let request = request.into_inner();

        let logs = request
            .resource_logs
            .into_iter()
            .flat_map(Log::try_from)
            .flatten()
            .collect();

        self.tx.send(logs).expect("to send log to storage");

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

    async fn get_logs(&self, deployment_id: String) -> Result<Vec<LogItem>, Error> {
        let logs = self.dal.get_logs(deployment_id.parse()?).await?;

        Ok(logs.into_iter().map(Into::into).collect())
    }
}

#[async_trait]
impl<D> Logger for Service<D>
where
    D: Dal + Send + Sync + 'static,
{
    async fn get_logs(
        &self,
        request: Request<LogsRequest>,
    ) -> Result<Response<LogsResponse>, Status> {
        // request.verify(Scope::Logs)?;

        let request = request.into_inner();
        let log_items = self.get_logs(request.deployment_id).await?;
        let result = LogsResponse { log_items };

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
