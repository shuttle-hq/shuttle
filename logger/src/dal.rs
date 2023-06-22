use std::{path::Path, str::FromStr, time::SystemTime};

use async_trait::async_trait;
use chrono::NaiveDateTime;
use opentelemetry_proto::tonic::{
    common::v1::{any_value, KeyValue},
    logs::v1::{LogRecord, ResourceLogs, ScopeLogs, SeverityNumber},
};
use prost_types::Timestamp;
use serde_json::Value;
use shuttle_common::tracing::{
    from_any_value_kv_to_serde_json_map, from_any_value_to_serde_json_value,
};
use shuttle_proto::logger::{self, LogItem};
use sqlx::{
    migrate::{MigrateDatabase, Migrator},
    sqlite::{SqliteConnectOptions, SqliteJournalMode},
    types::chrono::{DateTime, Utc},
    FromRow, QueryBuilder, SqlitePool,
};
use thiserror::Error;
use tokio::sync::broadcast;
use tracing::info;
use ulid::Ulid;

pub static MIGRATIONS: Migrator = sqlx::migrate!("./migrations");

#[derive(Error, Debug)]
pub enum DalError {
    #[error("database request failed: {0}")]
    Sqlx(#[from] sqlx::Error),
}

#[async_trait]
pub trait Dal {
    /// Get logs for a deployment
    async fn get_logs(&self, deployment_id: Ulid) -> Result<Vec<Log>, DalError>;
}

pub struct Sqlite {
    pool: SqlitePool,
    tx: broadcast::Sender<Vec<Log>>,
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

        let (tx, mut rx): (broadcast::Sender<Vec<Log>>, _) = broadcast::channel(256);
        let pool_spawn = pool.clone();

        tokio::spawn(async move {
            while let Ok(logs) = rx.recv().await {
                let mut builder = QueryBuilder::new("INSERT INTO logs (deployment_id, shuttle_service_name, timestamp, level, fields)");
                builder.push_values(logs, |mut b, log| {
                    b.push_bind(log.deployment_id)
                        .push_bind(log.shuttle_service_name)
                        .push_bind(log.timestamp)
                        .push_bind(log.level)
                        .push_bind(log.fields);
                });
                let query = builder.build();

                query.execute(&pool_spawn).await.unwrap();
            }
        });

        Self { pool, tx }
    }

    /// Get the sender to broadcast logs into
    pub fn get_sender(&self) -> broadcast::Sender<Vec<Log>> {
        self.tx.clone()
    }
}

#[async_trait]
impl Dal for Sqlite {
    async fn get_logs(&self, deployment_id: Ulid) -> Result<Vec<Log>, DalError> {
        let result = sqlx::query_as("SELECT * FROM logs WHERE deployment_id = ?")
            .bind(deployment_id.to_string())
            .fetch_all(&self.pool)
            .await?;

        Ok(result)
    }
}

#[derive(Clone, Debug, FromRow)]
pub struct Log {
    pub(crate) deployment_id: String,
    pub(crate) shuttle_service_name: String,
    pub(crate) timestamp: DateTime<Utc>,
    pub(crate) level: LogLevel,
    pub(crate) fields: Value,
}

#[derive(Clone, Debug, sqlx::Type)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl From<LogLevel> for logger::LogLevel {
    fn from(level: LogLevel) -> Self {
        match level {
            LogLevel::Trace => Self::Trace,
            LogLevel::Debug => Self::Debug,
            LogLevel::Info => Self::Info,
            LogLevel::Warn => Self::Warn,
            LogLevel::Error => Self::Error,
        }
    }
}

impl From<SeverityNumber> for LogLevel {
    fn from(severity: SeverityNumber) -> Self {
        match severity {
            SeverityNumber::Unspecified => Self::Trace,
            SeverityNumber::Trace
            | SeverityNumber::Trace2
            | SeverityNumber::Trace3
            | SeverityNumber::Trace4 => Self::Trace,
            SeverityNumber::Debug
            | SeverityNumber::Debug2
            | SeverityNumber::Debug3
            | SeverityNumber::Debug4 => Self::Debug,
            SeverityNumber::Info
            | SeverityNumber::Info2
            | SeverityNumber::Info3
            | SeverityNumber::Info4 => Self::Info,
            SeverityNumber::Warn
            | SeverityNumber::Warn2
            | SeverityNumber::Warn3
            | SeverityNumber::Warn4 => Self::Warn,
            SeverityNumber::Error
            | SeverityNumber::Error2
            | SeverityNumber::Error3
            | SeverityNumber::Error4
            | SeverityNumber::Fatal
            | SeverityNumber::Fatal2
            | SeverityNumber::Fatal3
            | SeverityNumber::Fatal4 => Self::Error,
        }
    }
}

impl Log {
    /// Try to get a log from an OTLP [ResourceLogs]
    pub fn try_from(logs: ResourceLogs) -> Option<Vec<Self>> {
        let ResourceLogs {
            resource,
            scope_logs,
            schema_url: _,
        } = logs;

        let shuttle_service_name = get_attribute(resource?.attributes, "service.name")?;

        let logs = scope_logs
            .into_iter()
            .flat_map(|log| {
                let ScopeLogs {
                    scope,
                    log_records,
                    schema_url: _,
                } = log;

                let deployment_id = get_attribute(scope?.attributes, "deployment_id")?;

                let logs: Vec<_> = log_records
                    .into_iter()
                    .flat_map(|log_record| {
                        Self::try_from_log_record(log_record, &shuttle_service_name, &deployment_id)
                    })
                    .collect();

                Some(logs)
            })
            .flatten()
            .collect();

        Some(logs)
    }

    /// Try to get self from an OTLP [LogRecord]. Also enrich it with the shuttle service name and deployment id.
    fn try_from_log_record(
        log_record: LogRecord,
        shuttle_service_name: &str,
        deployment_id: &str,
    ) -> Option<Self> {
        let level = log_record.severity_number().into();
        let naive = NaiveDateTime::from_timestamp_opt(
            (log_record.time_unix_nano / 1_000_000_000)
                .try_into()
                .unwrap_or_default(),
            (log_record.time_unix_nano % 1_000_000_000) as u32,
        )
        .unwrap_or_default();
        let mut fields = from_any_value_kv_to_serde_json_map(log_record.attributes);
        let message = from_any_value_to_serde_json_value(log_record.body?);

        fields.insert("message".to_string(), message);

        Some(Self {
            shuttle_service_name: shuttle_service_name.to_string(),
            deployment_id: deployment_id.to_string(),
            timestamp: DateTime::from_utc(naive, Utc),
            level,
            fields: Value::Object(fields),
        })
    }
}

impl From<Log> for LogItem {
    fn from(log: Log) -> Self {
        LogItem {
            service_name: log.shuttle_service_name,
            timestamp: Some(Timestamp::from(SystemTime::from(log.timestamp))),
            level: logger::LogLevel::from(log.level) as i32,
            fields: serde_json::to_vec(&log.fields).unwrap_or_default(),
        }
    }
}

/// Get an attribute with the given key
fn get_attribute(attributes: Vec<KeyValue>, key: &str) -> Option<String> {
    match attributes
        .into_iter()
        .find(|kv| kv.key == key)?
        .value?
        .value?
    {
        any_value::Value::StringValue(s) => Some(s),
        _ => None,
    }
}
