use std::{path::Path, str::FromStr, time::SystemTime};

use async_broadcast::{broadcast, Sender};
use async_trait::async_trait;
use chrono::NaiveDateTime;
use opentelemetry_proto::tonic::{
    common::v1::{any_value, KeyValue},
    trace::v1::{ResourceSpans, ScopeSpans, Span},
};
use prost_types::Timestamp;
use serde_json::Value;
use shuttle_common::{
    backends::tracing::from_any_value_kv_to_serde_json_map, tracing::MESSAGE_KEY,
};
use shuttle_proto::logger::{self, LogItem};
use sqlx::{
    migrate::{MigrateDatabase, Migrator},
    sqlite::{SqliteConnectOptions, SqliteJournalMode},
    types::chrono::{DateTime, Utc},
    FromRow, QueryBuilder, SqlitePool,
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
    async fn get_logs(&self, deployment_id: String) -> Result<Vec<Log>, DalError>;
}

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
                let mut builder = QueryBuilder::new("INSERT INTO logs (deployment_id, shuttle_service_name, timestamp, level, fields)");
                builder.push_values(logs, |mut b, log| {
                    b.push_bind(log.deployment_id)
                        .push_bind(log.shuttle_service_name)
                        .push_bind(log.timestamp)
                        .push_bind(log.level)
                        .push_bind(log.fields);
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
    async fn get_logs(&self, deployment_id: String) -> Result<Vec<Log>, DalError> {
        let result =
            sqlx::query_as("SELECT * FROM logs WHERE deployment_id = ? ORDER BY timestamp")
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

impl FromStr for LogLevel {
    type Err = DalError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "TRACE" => Ok(Self::Trace),
            "DEBUG" => Ok(Self::Debug),
            "INFO" => Ok(Self::Info),
            "WARN" => Ok(Self::Warn),
            "ERROR" => Ok(Self::Error),
            other => Err(DalError::Parsing(format!("invalid log level: {other}"))),
        }
    }
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

impl Log {
    /// Try to get a log from an OTLP [ResourceSpans]
    pub fn try_from_scope_span(resource_spans: ResourceSpans) -> Option<Vec<Self>> {
        let ResourceSpans {
            resource,
            scope_spans,
            schema_url: _,
        } = resource_spans;

        // TODO: we should get both of these attributes in the same function and avoid this clone.
        let resource = resource?;
        let shuttle_service_name = get_attribute(resource.clone().attributes, "service.name")?;

        // Try to get the deployment_id from the resource attributes, this will be the case for the runtimes,
        // they add the deployment_id to the otlp tracer config.
        // TODO: should this be named "deployment.id" to conform to otlp standard?
        let deployment_id = get_attribute(resource.attributes, "deployment_id");

        let logs = scope_spans
            .into_iter()
            .flat_map(|scope_spans| {
                let ScopeSpans {
                    spans,
                    schema_url: _,
                    ..
                } = scope_spans;

                let events: Vec<_> = spans
                    .into_iter()
                    .flat_map(|span| {
                        Self::try_from_span(span, &shuttle_service_name, deployment_id.clone())
                    })
                    .flatten()
                    .collect();

                Some(events)
            })
            .flatten()
            .collect();

        Some(logs)
    }

    /// Try to get self from an OTLP [Span]. Also enrich it with the shuttle service name and deployment id.
    fn try_from_span(
        span: Span,
        shuttle_service_name: &str,
        deployment_id: Option<String>,
    ) -> Option<Vec<Self>> {
        // If we didn't find the id in the resource span, check the inner spans.
        let deployment_id =
            deployment_id.or(get_attribute(span.attributes.clone(), "deployment_id"))?;

        let mut logs: Vec<Self> = span
            .events
            .into_iter()
            .flat_map(|event| {
                let message = event.name;

                let mut fields = from_any_value_kv_to_serde_json_map(event.attributes);
                fields.insert(MESSAGE_KEY.to_string(), message.into());

                // Since we store the "level" in the level column in the database, we remove it
                // from the event fields so it is not duplicated there.
                // Note: this should never fail, a tracing event should always have a level.
                let level = fields.remove("level")?;

                let naive = NaiveDateTime::from_timestamp_opt(
                    (event.time_unix_nano / 1_000_000_000)
                        .try_into()
                        .unwrap_or_default(),
                    (event.time_unix_nano % 1_000_000_000) as u32,
                )
                .unwrap_or_default();

                Some(Log {
                    shuttle_service_name: shuttle_service_name.to_string(),
                    deployment_id: deployment_id.to_string(),
                    timestamp: DateTime::from_utc(naive, Utc),
                    level: level.as_str()?.parse().ok()?,
                    fields: Value::Object(fields),
                })
            })
            .collect();

        let mut fields = from_any_value_kv_to_serde_json_map(span.attributes);
        fields.insert(
            MESSAGE_KEY.to_string(),
            format!("[span] {}", span.name).into(),
        );

        logs.push(Log {
            shuttle_service_name: shuttle_service_name.to_string(),
            deployment_id,
            timestamp: DateTime::from_utc(
                NaiveDateTime::from_timestamp_opt(
                    (span.start_time_unix_nano / 1_000_000_000)
                        .try_into()
                        .unwrap_or_default(),
                    (span.start_time_unix_nano % 1_000_000_000) as u32,
                )
                .unwrap_or_default(),
                Utc,
            ),
            // Span level doesn't exist so this info is not relevant.
            level: LogLevel::Info,
            fields: Value::Object(fields),
        });

        Some(logs)
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
