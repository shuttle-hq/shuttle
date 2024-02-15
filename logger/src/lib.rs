use async_trait::async_trait;
use dal::Log;
use dal::{Dal, DalError};
use shuttle_common::{backends::auth::VerifyClaim, claims::Scope};
use shuttle_proto::logger::LogLine;
use shuttle_proto::logger::{
    logger_server::Logger, LogsRequest, LogsResponse, StoreLogsRequest, StoreLogsResponse,
};
use thiserror::Error;
use tokio::sync::broadcast::Sender;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status};
use tracing::{debug, error, field, Span};

pub mod args;
mod dal;

pub use dal::Postgres;

/// A wrapper to capture any error possible with this service
#[derive(Error, Debug)]
pub enum Error {
    #[error("failed to interact with database: {0}")]
    Dal(#[from] DalError),
}

impl From<Error> for Status {
    fn from(error: Error) -> Self {
        Self::internal(error.to_string())
    }
}

pub struct Service<D> {
    dal: D,
    logs_tx: Sender<(Vec<Log>, Span)>,
}

impl<D> Service<D>
where
    D: Dal + Send + Sync + 'static,
{
    pub fn new(logs_tx: Sender<(Vec<Log>, Span)>, dal: D) -> Self {
        Self { dal, logs_tx }
    }

    async fn get_logs(&self, deployment_id: String) -> Result<Vec<LogLine>, Error> {
        let logs = self.dal.get_logs(deployment_id).await?;

        Ok(logs.into_iter().map(Into::into).collect())
    }
}

#[async_trait]
impl<D> Logger for Service<D>
where
    D: Dal + Send + Sync + 'static,
{
    #[tracing::instrument(skip(self, request), fields(deployment_id = field::Empty, batch_size = field::Empty))]
    async fn store_logs(
        &self,
        request: Request<StoreLogsRequest>,
    ) -> Result<Response<StoreLogsResponse>, Status> {
        let request = request.into_inner();
        let logs = request.logs;

        if !logs.is_empty() {
            let span = Span::current();
            span.record("deployment_id", &logs[0].deployment_id);
            span.record("batch_size", logs.len());

            _ = self
                .logs_tx
                .send((
                    logs.into_iter().filter_map(Log::from_log_item).collect(),
                    span,
                ))
                .map_err(|err| {
                    Status::internal(format!(
                        "Errored while trying to store the logs in persistence: {err}"
                    ))
                })?;
        }

        Ok(Response::new(StoreLogsResponse { success: true }))
    }

    #[tracing::instrument(skip(self))]
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

    type GetLogsStreamStream = ReceiverStream<Result<LogLine, Status>>;

    #[tracing::instrument(skip(self))]
    async fn get_logs_stream(
        &self,
        request: Request<LogsRequest>,
    ) -> Result<Response<Self::GetLogsStreamStream>, Status> {
        request.verify(Scope::Logs)?;

        // Subscribe as soon as possible
        let mut logs_rx = self.logs_tx.subscribe();
        let LogsRequest { deployment_id } = request.into_inner();
        let (tx, rx) = mpsc::channel(1);

        // Get logs before stream was started
        let logs = self.get_logs(deployment_id.clone()).await?;

        tokio::spawn(async move {
            let mut last = Default::default();

            for log in logs {
                last = log.tx_timestamp.clone().unwrap_or_default();
                if let Err(error) = tx.send(Ok(log)).await {
                    error!(
                        error = &error as &dyn std::error::Error,
                        "error sending past log"
                    );

                    // Receiver closed so end stream spawn
                    return;
                };
            }

            loop {
                match logs_rx.recv().await {
                    Ok((logs, _span)) => {
                        if !logs_rx.is_empty() {
                            debug!("stream receiver queue size {}", logs_rx.len())
                        }

                        for log in logs {
                            if log.deployment_id == deployment_id
                                && log.tx_timestamp.timestamp() >= last.seconds
                                && log.tx_timestamp.timestamp_nanos_opt().unwrap_or_default()
                                    > last.nanos.into()
                            {
                                if let Err(error) = tx.send(Ok(log.into())).await {
                                    error!(
                                        error = &error as &dyn std::error::Error,
                                        "error sending new log"
                                    );

                                    // Receiver closed so end stream spawn
                                    return;
                                };
                            }
                        }
                    }
                    Err(error) => {
                        error!(
                            error = &error as &dyn std::error::Error,
                            "failed to receive logs in logs stream"
                        );
                    }
                }
            }
        });

        Ok(Response::new(ReceiverStream::new(rx)))
    }
}
