use std::{path::Path, str::FromStr, time::SystemTime};

use async_broadcast::{broadcast, Sender};
use async_trait::async_trait;
use chrono::NaiveDateTime;
use prost_types::Timestamp;
use shuttle_proto::logger::{LogItem, LogLine};
use sqlx::{
    migrate::{MigrateDatabase, Migrator},
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqliteRow},
    types::chrono::{DateTime, Utc},
    FromRow, QueryBuilder, Row, SqlitePool,
};
use thiserror::Error;
use tracing::{error, info};

pub static MIGRATIONS: Migrator = sqlx::migrate!("./migrations");

#[derive(Error, Debug)]
pub enum DalError {
    #[error("database request failed: {0}")]
    Sqlx(#[from] sqlx::Error),
    #[error("parsing log failed: {0}")]
    Parsing(String),
}

#[async_trait]
pub trait Dal {
    /// Get logs for a deployment
    async fn get_logs(
        &self,
        deployment_id: String,
        page: Option<u32>,
        limit: Option<u32>,
    ) -> Result<Vec<Log>, DalError>;
}

#[derive(Clone)]
pub struct Sqlite {
    pool: SqlitePool,
    tx: Sender<Vec<Log>>,
}

impl Sqlite {
    /// This function creates all necessary tables and sets up a database connection pool.
    pub async fn new(path: &str) -> Self {
        if !Path::new(path).exists() {
            sqlx::Sqlite::create_database(path).await.unwrap();
        }

        info!(
            "state db: {}",
            std::fs::canonicalize(path).unwrap().to_string_lossy()
        );

        // We have found in the past that setting synchronous to anything other than the default (full) breaks the
        // broadcast channel in deployer. The broken symptoms are that the ws socket connections won't get any logs
        // from the broadcast channel and would then close. When users did deploys, this would make it seem like the
        // deploy is done (while it is still building for most of the time) and the status of the previous deployment
        // would be returned to the user.
        //
        // If you want to activate a faster synchronous mode, then also do proper testing to confirm this bug is no
        // longer present.
        let sqlite_options = SqliteConnectOptions::from_str(path)
            .unwrap()
            .journal_mode(SqliteJournalMode::Wal);

        let pool = SqlitePool::connect_with(sqlite_options).await.unwrap();

        Self::from_pool(pool).await
    }

    #[allow(dead_code)]
    pub async fn new_in_memory() -> Self {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        Self::from_pool(pool).await
    }

    async fn from_pool(pool: SqlitePool) -> Self {
        MIGRATIONS.run(&pool).await.unwrap();

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
impl Dal for Sqlite {
    async fn get_logs(
        &self,
        deployment_id: String,
        page: Option<u32>,
        limit: Option<u32>,
    ) -> Result<Vec<Log>, DalError> {
        let mut query = QueryBuilder::new("SELECT * FROM logs WHERE deployment_id = ");
        query
            .push_bind(deployment_id)
            .push("  ORDER BY tx_timestamp");
        if let Some(limit) = limit {
            query.push(" LIMIT ").push_bind(limit);
            if let Some(page) = page {
                let offset = page * limit;
                if offset > 0 {
                    query.push(" OFFSET ").push_bind(offset);
                }
            }
        }
        let result = query
            .build()
            .fetch_all(&self.pool)
            .await?
            .iter()
            .map(Log::from)
            .collect();
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

impl<'a> From<&'a SqliteRow> for Log {
    fn from(row: &'a SqliteRow) -> Self {
        Self {
            deployment_id: row.get("deployment_id"),
            shuttle_service_name: row.get("shuttle_service_name"),
            tx_timestamp: row.get("tx_timestamp"),
            data: row.get("data"),
        }
    }
}

impl Log {
    pub(crate) fn from_log_item(log: LogItem) -> Option<Self> {
        let log_line = log.log_line?;
        let timestamp = log_line.tx_timestamp.clone().unwrap_or_default();
        Some(Log {
            deployment_id: log.deployment_id,
            shuttle_service_name: log_line.service_name,
            tx_timestamp: DateTime::from_utc(
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
