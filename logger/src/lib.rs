use async_broadcast::Sender;
use async_trait::async_trait;
use chrono::{DateTime, NaiveDateTime, Utc};
use dal::{Dal, DalError};
use shuttle_common::{backends::auth::VerifyClaim, claims::Scope};
use shuttle_proto::logger::{
    logger_server::Logger, LogItem, LogsRequest, LogsResponse, StoreLogsRequest, StoreLogsResponse,
};
use thiserror::Error;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status};

pub mod args;
mod dal;

pub use dal::Log;
pub use dal::Sqlite;

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
    logs_tx: Sender<Vec<Log>>,
}

impl<D> Service<D>
where
    D: Dal + Send + Sync + 'static,
{
    pub fn new(logs_tx: Sender<Vec<Log>>, dal: D) -> Self {
        Self { dal, logs_tx }
    }

    async fn get_logs(&self, deployment_id: String) -> Result<Vec<LogItem>, Error> {
        let logs = self.dal.get_logs(deployment_id).await?;

        Ok(logs.into_iter().map(Into::into).collect())
    }
}

#[async_trait]
impl<D> Logger for Service<D>
where
    D: Dal + Send + Sync + 'static,
{
    async fn store_logs(
        &self,
        request: Request<StoreLogsRequest>,
    ) -> Result<Response<StoreLogsResponse>, Status> {
        request.verify(Scope::DeploymentPush)?;

        let request = request.into_inner();
        let logs = request.logs;
        if !logs.is_empty() {
            _ = self
                .logs_tx
                .broadcast(
                    logs.into_iter()
                        .map(|li| {
                            let timestamp = li.tx_timestamp.clone().unwrap_or_default();
                            Log {
                                deployment_id: li.deployment_id,
                                shuttle_service_name: li.service_name,
                                tx_timestamp: DateTime::from_utc(
                                    NaiveDateTime::from_timestamp_opt(
                                        timestamp.seconds,
                                        timestamp.nanos.try_into().unwrap_or_default(),
                                    )
                                    .unwrap_or_default(),
                                    Utc,
                                ),
                                data: li.data,
                            }
                        })
                        .collect(),
                )
                .await
                .map_err(|err| {
                    println!("failed to send log to storage: {}", err);
                });
        }

        Ok(Response::new(StoreLogsResponse { success: true }))
    }

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
        let mut logs_rx = self.logs_tx.new_receiver();
        let request = request.into_inner();
        let (tx, rx) = mpsc::channel(1);
        let logs = self.get_logs(request.deployment_id).await?;

        tokio::spawn(async move {
            let mut last = Default::default();

            for log in logs {
                last = log.tx_timestamp.clone().unwrap_or_default();
                if let Err(error) = tx.send(Ok(log)).await {
                    println!("error sending log: {}", error);
                };
            }

            while let Ok(logs) = logs_rx.recv().await {
                for log in logs {
                    if log.tx_timestamp.timestamp() >= last.seconds
                        && log.tx_timestamp.timestamp_nanos() > last.nanos.into()
                    {
                        tx.send(Ok(log.into())).await.unwrap();
                    }
                }
            }
        });

        Ok(Response::new(ReceiverStream::new(rx)))
    }
}
