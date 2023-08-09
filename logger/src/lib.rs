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

pub mod args;
mod dal;

pub use dal::Sqlite;

/// A wrapper to capture any error possible with this service
#[derive(Error, Debug)]
pub enum Error {
    #[error("could not parse id: {0}")]
    Uuid(#[from] uuid::Error),

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
    logs_tx: broadcast::Sender<Vec<Log>>,
}

impl<D> Service<D>
where
    D: Dal + Send + Sync + 'static,
{
    pub fn new(logs_rx: broadcast::Sender<Vec<Log>>, dal: D) -> Self {
        Self {
            dal,
            logs_tx: logs_rx,
        }
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
        request.verify(Scope::Logs)?;

        let request = request.into_inner();
        let log_items = self.get_logs(request.deployment_id).await?;
        let result = LogsResponse { log_items };

        Ok(Response::new(result))
    }

    type GetLogsStreamStream = ReceiverStream<Result<LogItem, Status>>;

    async fn get_logs_stream(
        &self,
        request: Request<LogsRequest>,
    ) -> Result<Response<Self::GetLogsStreamStream>, Status> {
        request.verify(Scope::Logs)?;

        // Subscribe as soon as possible
        let mut logs_rx = self.logs_tx.subscribe();
        let request = request.into_inner();
        let (tx, rx) = mpsc::channel(1);
        let logs = self.get_logs(request.deployment_id).await?;

        tokio::spawn(async move {
            let mut last = Default::default();

            for log in logs {
                last = log.timestamp.clone().unwrap_or_default();
                tx.send(Ok(log)).await.unwrap();
            }

            while let Ok(logs) = logs_rx.recv().await {
                for log in logs {
                    let log: LogItem = log.into();
                    let this_time = log.timestamp.clone().unwrap_or_default();

                    if this_time.seconds >= last.seconds && this_time.nanos > last.nanos {
                        tx.send(Ok(log)).await.unwrap();
                    }
                }
            }
        });

        Ok(Response::new(ReceiverStream::new(rx)))
    }
}
