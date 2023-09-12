use std::time::SystemTime;

use async_broadcast::{broadcast, Sender};
use async_trait::async_trait;
use chrono::NaiveDateTime;
use prost_types::Timestamp;
use shuttle_proto::logger::{LogItem, LogLine};
use sqlx::{
    migrate::Migrator,
    postgres::PgConnectOptions,
    types::chrono::{DateTime, Utc},
    FromRow, PgPool, QueryBuilder,
};
use thiserror::Error;
use tracing::error;

use tonic::transport::Uri;

pub static MIGRATIONS: Migrator = sqlx::migrate!("./migrations");

#[derive(Error, Debug)]
pub enum DalError {
    #[error("database request failed: {0}")]
    Sqlx(#[from] sqlx::Error),
}

#[async_trait]
pub trait Dal {
    /// Get logs for a deployment
    async fn get_logs(&self, deployment_id: String) -> Result<Vec<Log>, DalError>;
}

#[derive(Clone)]
pub struct Postgres {
    pool: PgPool,
    tx: Sender<Vec<Log>>,
}

impl Postgres {
    /// This function creates all necessary tables and sets up a database connection pool.
    pub async fn new(connection_uri: &Uri) -> Self {
        let pool = PgPool::connect(connection_uri.to_string().as_str())
            .await
            .expect("to be able to connect to the postgres db using the connection url");
        Self::from_pool(pool).await
    }

    pub async fn with_options(options: PgConnectOptions) -> Self {
        let pool = PgPool::connect_with(options)
            .await
            .expect("to be able to connect to the postgres db using the pg connect options");
        Self::from_pool(pool).await
    }

    async fn from_pool(pool: PgPool) -> Self {
        MIGRATIONS
            .run(&pool)
            .await
            .expect("to run migrations successfully");

        // TODO: we switched to async_broadcast to resolve the infinite loop bug, but it wasn't related.
        // Should we switch back to tokio::broadcast?
        let (tx, mut rx): (Sender<Vec<Log>>, _) = broadcast(1000);
        let pool_spawn = pool.clone();

        tokio::spawn(async move {
            while let Ok(logs) = rx.recv().await {
                let mut builder = QueryBuilder::new(
                    "INSERT INTO logs (deployment_id, shuttle_service_name, data, tx_timestamp)",
                );
                builder.push_values(logs, |mut b, log| {
                    b.push_bind(log.deployment_id)
                        .push_bind(log.shuttle_service_name)
                        .push_bind(log.data)
                        .push_bind(log.tx_timestamp);
                });
                let query = builder.build();

                if let Err(error) = query.execute(&pool_spawn).await {
                    error!(error = %error, "failed to insert logs");
                };
            }
        });

        Self { pool, tx }
    }

    /// Get the sender to broadcast logs into
    pub fn get_sender(&self) -> Sender<Vec<Log>> {
        self.tx.clone()
    }
}

#[async_trait]
impl Dal for Postgres {
    async fn get_logs(&self, deployment_id: String) -> Result<Vec<Log>, DalError> {
        let result =
            sqlx::query_as("SELECT * FROM logs WHERE deployment_id = $1 ORDER BY tx_timestamp")
                .bind(deployment_id)
                .fetch_all(&self.pool)
                .await?;

        Ok(result)
    }
}

#[derive(Clone, Debug, FromRow)]
pub struct Log {
    pub(crate) deployment_id: String,
    pub(crate) shuttle_service_name: String,
    pub(crate) tx_timestamp: DateTime<Utc>,
    pub(crate) data: Vec<u8>,
}

impl Log {
    pub(crate) fn from_log_item(log: LogItem) -> Option<Self> {
        let log_line = log.log_line?;
        let timestamp = log_line.tx_timestamp.clone().unwrap_or_default();
        Some(Log {
            deployment_id: log.deployment_id,
            shuttle_service_name: log_line.service_name,
            tx_timestamp: DateTime::from_naive_utc_and_offset(
                NaiveDateTime::from_timestamp_opt(
                    timestamp.seconds,
                    timestamp.nanos.try_into().unwrap_or_default(),
                )
                .unwrap_or_default(),
                Utc,
            ),
            data: log_line.data,
        })
    }
}

impl From<Log> for LogItem {
    fn from(log: Log) -> Self {
        LogItem {
            deployment_id: log.deployment_id,
            log_line: Some(LogLine {
                service_name: log.shuttle_service_name,
                tx_timestamp: Some(Timestamp::from(SystemTime::from(log.tx_timestamp))),
                data: log.data,
            }),
        }
    }
}

impl From<Log> for LogLine {
    fn from(log: Log) -> Self {
        LogLine {
            service_name: log.shuttle_service_name,
            tx_timestamp: Some(Timestamp::from(SystemTime::from(log.tx_timestamp))),
            data: log.data,
        }
    }
}
