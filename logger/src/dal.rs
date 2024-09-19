use core::fmt;
use std::time::{Duration, SystemTime};

use async_trait::async_trait;
use chrono::NaiveDateTime;
use prost_types::Timestamp;
use shuttle_common::log::LogsRange;
use shuttle_proto::logger::{LogItem, LogLine};
use sqlx::{
    migrate::Migrator,
    types::chrono::{DateTime, Utc},
    Executor, FromRow, PgPool, QueryBuilder,
};
use thiserror::Error;
use tokio::sync::broadcast::{self, Sender};
use tracing::{error, info, warn, Instrument, Span};

use tonic::transport::Uri;

pub static MIGRATIONS: Migrator = sqlx::migrate!("./migrations");

#[derive(Error, Debug)]
pub enum DalError {
    Sqlx(#[from] sqlx::Error),
}

// We are not using the `thiserror`'s `#[error]` syntax to prevent sensitive details from bubbling up to the users.
// Instead we are logging it as an error which we can inspect.
impl fmt::Display for DalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let msg = match self {
            DalError::Sqlx(error) => {
                error!(
                    error = error as &dyn std::error::Error,
                    "database request failed"
                );

                "failed to interact with logger"
            }
        };

        write!(f, "{msg}")
    }
}

#[async_trait]
pub trait Dal {
    /// Get logs for a deployment
    async fn get_logs(
        &self,
        deployment_id: String,
        head: Option<u32>,
        tail: Option<u32>,
    ) -> Result<Vec<Log>, DalError>;
}

#[derive(Clone)]
pub struct Postgres {
    pool: PgPool,
    tx: Sender<(Vec<Log>, Span)>,
}

impl Postgres {
    /// This function creates all necessary tables and sets up a database connection pool.
    pub async fn new(connection_uri: &Uri) -> Self {
        let pool = PgPool::connect(connection_uri.to_string().as_str())
            .await
            .expect("to connect to the db");
        Self::from_pool(pool).await
    }

    async fn from_pool(pool: PgPool) -> Self {
        MIGRATIONS
            .run(&pool)
            .await
            .expect("to run migrations successfully");

        let pool_clone = pool.clone();
        tokio::spawn(async move {
            info!("cleaning old logs");
            pool_clone
                .execute("DELETE FROM logs WHERE tx_timestamp < (NOW() - INTERVAL '1 month')")
                .await
                .expect("to clean old logs successfully");
            info!("done cleaning old logs");
        });

        let (tx, mut rx) = broadcast::channel::<(Vec<Log>, Span)>(1000);

        let interval_tx = tx.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(10));

            loop {
                interval.tick().await;
                info!("broadcast channel queue size: {}", interval_tx.len());
            }
        });

        let pool_clone = pool.clone();
        tokio::spawn(async move {
            loop {
                match rx.recv().await {
                    Ok((logs, parent_span)) => {
                        let mut builder = QueryBuilder::new(
                            "INSERT INTO logs (deployment_id, shuttle_service_name, data, tx_timestamp)",
                        );

                        parent_span.in_scope(|| {
                            if rx.len() >= 200 {
                                warn!(
                                    queue_size = rx.len(),
                                    "database receiver queue is filling up"
                                );
                            } else if !rx.is_empty() {
                                info!("database receiver queue size: {}", rx.len());
                            }
                        });

                        builder.push_values(logs, |mut b, log| {
                            b.push_bind(log.deployment_id)
                                .push_bind(log.shuttle_service_name)
                                .push_bind(log.data)
                                .push_bind(log.tx_timestamp);
                        });
                        let query = builder.build();

                        if let Err(error) = query.execute(&pool_clone).instrument(parent_span).await
                        {
                            error!(
                                error = &error as &dyn std::error::Error,
                                "failed to insert logs"
                            );
                        };
                    }
                    Err(err) => {
                        error!(
                            error = &err as &dyn std::error::Error,
                            "failed to receive message in database receiver"
                        );
                    }
                }
            }
        });

        Self { pool, tx }
    }

    /// Get the sender to broadcast logs into
    pub fn get_sender(&self) -> Sender<(Vec<Log>, Span)> {
        self.tx.clone()
    }
}

#[async_trait]
impl Dal for Postgres {
    async fn get_logs(
        &self,
        deployment_id: String,
        head: Option<u32>,
        tail: Option<u32>,
    ) -> Result<Vec<Log>, DalError> {
        let mode = match (head, tail) {
            (Some(len), None) => LogsRange::Head(len),
            (None, Some(len)) => LogsRange::Tail(len),
            (None, None) => LogsRange::All,
            _ => LogsRange::Tail(1000),
        };

        let result = match mode {
            LogsRange::Head(len) => {
                sqlx::query_as("SELECT * FROM logs WHERE deployment_id = $1 ORDER BY tx_timestamp limit $2")
                    .bind(deployment_id)
                    .bind(len as i64)
                    .fetch_all(&self.pool)
                    .await?
            }
            LogsRange::Tail(len) => {
                sqlx::query_as("SELECT * FROM (SELECT * FROM logs WHERE deployment_id = $1 ORDER BY tx_timestamp DESC limit $2) AS TAIL_TABLE ORDER BY tx_timestamp")
                    .bind(deployment_id)
                    .bind(len as i64)
                    .fetch_all(&self.pool)
                    .await?
            }
            LogsRange::All => {
                sqlx::query_as("SELECT * FROM logs WHERE deployment_id = $1 ORDER BY tx_timestamp")
                    .bind(deployment_id)
                    .fetch_all(&self.pool)
                    .await?
            }
        };
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
