mod deployment;
mod error;
mod log;
mod resource;
mod secret;
mod state;

use crate::deployment::deploy_layer::{self, LogRecorder, LogType};
use error::{Error, Result};

use std::path::Path;

use chrono::Utc;
use serde_json::json;
use shuttle_common::STATE_MESSAGE;
use sqlx::migrate::MigrateDatabase;
use sqlx::sqlite::{Sqlite, SqlitePool};
use tokio::sync::broadcast::{self, Receiver, Sender};
use tokio::task::JoinHandle;
use tracing::error;
use uuid::Uuid;

use self::deployment::DeploymentRunnable;
pub use self::deployment::{Deployment, DeploymentState};
pub use self::error::Error as PersistenceError;
pub use self::log::{Level as LogLevel, Log};
pub use self::resource::{Resource, ResourceRecorder, Type as ResourceType};
use self::secret::Secret;
pub use self::secret::{SecretGetter, SecretRecorder};
pub use self::state::State;

const DB_PATH: &str = "deployer.sqlite";

#[derive(Clone)]
pub struct Persistence {
    pool: SqlitePool,
    log_send: crossbeam_channel::Sender<deploy_layer::Log>,
    stream_log_send: Sender<deploy_layer::Log>,
}

impl Persistence {
    /// Creates a persistent storage solution (i.e., SQL database). This
    /// function creates all necessary tables and sets up a database connection
    /// pool - new connections should be made by cloning [`Persistence`] rather
    /// than repeatedly calling [`Persistence::new`].
    pub async fn new() -> (Self, JoinHandle<()>) {
        if !Path::new(DB_PATH).exists() {
            Sqlite::create_database(DB_PATH).await.unwrap();
        }

        let pool = SqlitePool::connect(DB_PATH).await.unwrap();
        Self::from_pool(pool).await
    }

    #[allow(dead_code)]
    async fn new_in_memory() -> (Self, JoinHandle<()>) {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        Self::from_pool(pool).await
    }

    async fn from_pool(pool: SqlitePool) -> (Self, JoinHandle<()>) {
        sqlx::query("
            CREATE TABLE IF NOT EXISTS deployments (
                id TEXT PRIMARY KEY, -- Identifier of the deployment.
                name TEXT,           -- Name of the service being deployed.
                state TEXT,          -- Enum indicating the current state of the deployment.
                last_update INTEGER  -- Unix epoch of the last status update
            );

            CREATE TABLE IF NOT EXISTS logs (
                id TEXT,           -- The deployment that this log line pertains to.
                timestamp INTEGER, -- Unix epoch timestamp.
                state TEXT,        -- The state of the deployment at the time at which the log text was produced.
                level TEXT,        -- The log level
                file TEXT,         -- The file log took place in
                line INTEGER,      -- The line log took place on
                target TEXT,       -- The module log took place in
                fields TEXT,       -- Log fields object.
                PRIMARY KEY (id, timestamp)
            );

            CREATE TABLE IF NOT EXISTS resources (
                name TEXT,         -- Name of the service this resource belongs to.
                type TEXT,         -- Type of resource this is.
                data TEXT,         -- Data about this resource.
                PRIMARY KEY (name, type)
            );

            CREATE TABLE IF NOT EXISTS secrets (
                name TEXT,            -- Name of the service this secret belongs to.
                key TEXT,             -- Key / name of this secret.
                value TEXT,           -- The actual secret.
                last_update INTEGER,  -- Unix epoch of the last secret update
                PRIMARY KEY (name, key)
            );
        ").execute(&pool).await.unwrap();

        let (log_send, log_recv): (crossbeam_channel::Sender<deploy_layer::Log>, _) =
            crossbeam_channel::bounded(0);

        let (stream_log_send, _) = broadcast::channel(32);
        let stream_log_send_clone = stream_log_send.clone();

        let pool_cloned = pool.clone();

        // The logs are received on a non-async thread.
        // This moves them to an async thread
        let handle = tokio::spawn(async move {
            while let Ok(log) = log_recv.recv() {
                if stream_log_send_clone.receiver_count() > 0 {
                    stream_log_send_clone
                        .send(log.clone())
                        .unwrap_or_else(|error| {
                            error!(
                                error = &error as &dyn std::error::Error,
                                "failed to broadcast log"
                            );

                            0
                        });
                }

                match log.r#type {
                    LogType::Event => {
                        insert_log(&pool_cloned, log).await.unwrap_or_else(|error| {
                            error!(
                                error = &error as &dyn std::error::Error,
                                "failed to insert event log"
                            )
                        });
                    }
                    LogType::State => {
                        insert_log(
                            &pool_cloned,
                            Log {
                                id: log.id,
                                timestamp: log.timestamp,
                                state: log.state,
                                level: log.level.clone(),
                                file: log.file.clone(),
                                line: log.line,
                                target: String::new(),
                                fields: json!(STATE_MESSAGE),
                            },
                        )
                        .await
                        .unwrap_or_else(|error| {
                            error!(
                                error = &error as &dyn std::error::Error,
                                "failed to insert state log"
                            )
                        });
                        update_deployment(&pool_cloned, log)
                            .await
                            .unwrap_or_else(|error| {
                                error!(
                                    error = &error as &dyn std::error::Error,
                                    "failed to update deployment state"
                                )
                            });
                    }
                };
            }
        });

        let persistence = Self {
            pool,
            log_send,
            stream_log_send,
        };

        (persistence, handle)
    }

    pub async fn insert_deployment(&self, deployment: impl Into<Deployment>) -> Result<()> {
        let deployment = deployment.into();

        sqlx::query("INSERT INTO deployments (id, name, state, last_update) VALUES (?, ?, ?, ?)")
            .bind(deployment.id)
            .bind(deployment.name)
            .bind(deployment.state)
            .bind(deployment.last_update)
            .execute(&self.pool)
            .await
            .map(|_| ())
            .map_err(Error::from)
    }

    pub async fn get_deployment(&self, id: &Uuid) -> Result<Option<Deployment>> {
        get_deployment(&self.pool, id).await
    }

    pub async fn get_deployments(&self, name: &str) -> Result<Vec<Deployment>> {
        sqlx::query_as("SELECT * FROM deployments WHERE name = ?")
            .bind(name)
            .fetch_all(&self.pool)
            .await
            .map_err(Error::from)
    }

    pub async fn get_active_deployment(&self, name: &str) -> Result<Option<Deployment>> {
        sqlx::query_as("SELECT * FROM deployments WHERE name = ? AND state = ?")
            .bind(name)
            .bind(State::Running)
            .fetch_optional(&self.pool)
            .await
            .map_err(Error::from)
    }

    pub async fn delete_service(&self, name: &str) -> Result<Vec<Deployment>> {
        let deployments = self.get_deployments(name).await?;

        let _ = sqlx::query("DELETE FROM deployments WHERE name = ?")
            .bind(name)
            .execute(&self.pool)
            .await;

        Ok(deployments)
    }

    pub async fn get_all_services(&self) -> Result<Vec<String>> {
        sqlx::query_as::<_, (String,)>("SELECT UNIQUE(name) FROM deployments")
            .fetch_all(&self.pool)
            .await
            .map_err(Error::from)
            .map(|vec| vec.into_iter().map(|t| t.0).collect())
    }

    pub async fn get_all_runnable_deployments(&self) -> Result<Vec<DeploymentRunnable>> {
        sqlx::query_as(
            r#"SELECT id, name, max(last_update) as last_update FROM deployments WHERE state = ? GROUP BY name"#,
        )
        .bind(State::Running)
        .fetch_all(&self.pool)
        .await
        .map_err(Error::from)
    }

    pub async fn get_service_resources(&self, name: &str) -> Result<Vec<Resource>> {
        sqlx::query_as(r#"SELECT * FROM resources WHERE name = ?"#)
            .bind(name)
            .fetch_all(&self.pool)
            .await
            .map_err(Error::from)
    }

    pub(crate) async fn get_deployment_logs(&self, id: &Uuid) -> Result<Vec<Log>> {
        // TODO: stress this a bit
        get_deployment_logs(&self.pool, id).await
    }

    pub fn get_log_subscriber(&self) -> Receiver<deploy_layer::Log> {
        self.stream_log_send.subscribe()
    }

    pub fn get_log_sender(&self) -> crossbeam_channel::Sender<deploy_layer::Log> {
        self.log_send.clone()
    }
}

async fn update_deployment(pool: &SqlitePool, state: impl Into<DeploymentState>) -> Result<()> {
    let state = state.into();

    // TODO: Handle moving to 'active_deployments' table for State::Running.

    sqlx::query("UPDATE deployments SET state = ?, last_update = ? WHERE id = ?")
        .bind(state.state)
        .bind(state.last_update)
        .bind(state.id)
        .execute(pool)
        .await
        .map(|_| ())
        .map_err(Error::from)
}

async fn get_deployment(pool: &SqlitePool, id: &Uuid) -> Result<Option<Deployment>> {
    sqlx::query_as("SELECT * FROM deployments WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await
        .map_err(Error::from)
}

async fn insert_log(pool: &SqlitePool, log: impl Into<Log>) -> Result<()> {
    let log = log.into();

    sqlx::query("INSERT INTO logs (id, timestamp, state, level, file, line, target, fields) VALUES (?, ?, ?, ?, ?, ?, ?, ?)")
        .bind(log.id)
        .bind(log.timestamp)
        .bind(log.state)
        .bind(log.level)
        .bind(log.file)
        .bind(log.line)
        .bind(log.target)
        .bind(log.fields)
        .execute(pool)
        .await
        .map(|_| ())
        .map_err(Error::from)
}

async fn get_deployment_logs(pool: &SqlitePool, id: &Uuid) -> Result<Vec<Log>> {
    sqlx::query_as("SELECT * FROM logs WHERE id = ? ORDER BY timestamp")
        .bind(id)
        .fetch_all(pool)
        .await
        .map_err(Error::from)
}

impl LogRecorder for Persistence {
    fn record(&self, log: deploy_layer::Log) {
        self.log_send
            .send(log)
            .expect("failed to move log to async thread");
    }
}

#[async_trait::async_trait]
impl ResourceRecorder for Persistence {
    type Err = Error;

    async fn insert_resource(&self, resource: &Resource) -> Result<()> {
        sqlx::query("INSERT OR REPLACE INTO resources (name, type, data) VALUES (?, ?, ?)")
            .bind(&resource.name)
            .bind(resource.r#type)
            .bind(&resource.data)
            .execute(&self.pool)
            .await
            .map(|_| ())
            .map_err(Error::from)
    }
}

#[async_trait::async_trait]
impl SecretRecorder for Persistence {
    type Err = Error;

    async fn insert_secret(&self, name: &str, key: &str, value: &str) -> Result<()> {
        sqlx::query(
            "INSERT OR REPLACE INTO secrets (name, key, value, last_update) VALUES (?, ?, ?, ?)",
        )
        .bind(name)
        .bind(key)
        .bind(value)
        .bind(Utc::now())
        .execute(&self.pool)
        .await
        .map(|_| ())
        .map_err(Error::from)
    }
}

#[async_trait::async_trait]
impl SecretGetter for Persistence {
    type Err = Error;

    async fn get_secrets(&self, name: &str) -> Result<Vec<Secret>> {
        sqlx::query_as("SELECT * FROM secrets WHERE name = ? ORDER BY key")
            .bind(name)
            .fetch_all(&self.pool)
            .await
            .map_err(Error::from)
    }
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};
    use serde_json::json;

    use super::*;
    use crate::persistence::{
        deployment::{Deployment, DeploymentRunnable, DeploymentState},
        log::{Level, Log},
        state::State,
    };

    #[tokio::test(flavor = "multi_thread")]
    async fn deployment_updates() {
        let (p, _) = Persistence::new_in_memory().await;

        let id = Uuid::new_v4();
        let deployment = Deployment {
            id,
            name: "abc".to_string(),
            state: State::Queued,
            last_update: Utc.ymd(2022, 4, 25).and_hms(4, 43, 33),
        };

        p.insert_deployment(deployment.clone()).await.unwrap();
        assert_eq!(p.get_deployment(&id).await.unwrap().unwrap(), deployment);

        update_deployment(
            &p.pool,
            DeploymentState {
                id,
                state: State::Built,
                last_update: Utc::now(),
            },
        )
        .await
        .unwrap();
        let update = p.get_deployment(&id).await.unwrap().unwrap();
        assert_eq!(update.state, State::Built);
        assert_ne!(update.last_update, Utc.ymd(2022, 4, 25).and_hms(4, 43, 33));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn deployment_active() {
        let (p, _) = Persistence::new_in_memory().await;

        let deployment_crashed = Deployment {
            id: Uuid::new_v4(),
            name: "xyz".to_string(),
            state: State::Crashed,
            last_update: Utc.ymd(2022, 4, 25).and_hms(7, 29, 35),
        };
        let deployment_stopped = Deployment {
            id: Uuid::new_v4(),
            name: "xyz".to_string(),
            state: State::Stopped,
            last_update: Utc.ymd(2022, 4, 25).and_hms(7, 49, 35),
        };
        let deployment_other = Deployment {
            id: Uuid::new_v4(),
            name: "other".to_string(),
            state: State::Running,
            last_update: Utc.ymd(2022, 4, 25).and_hms(7, 39, 39),
        };
        let deployment_running = Deployment {
            id: Uuid::new_v4(),
            name: "xyz".to_string(),
            state: State::Running,
            last_update: Utc.ymd(2022, 4, 25).and_hms(7, 48, 29),
        };

        for deployment in [
            &deployment_crashed,
            &deployment_stopped,
            &deployment_other,
            &deployment_running,
        ] {
            p.insert_deployment(deployment.clone()).await.unwrap();
        }

        assert_eq!(
            p.get_active_deployment("xyz").await.unwrap().unwrap(),
            deployment_running
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn fetching_runnable_deployments() {
        let (p, _) = Persistence::new_in_memory().await;

        let id_bar = Uuid::new_v4();
        let id_foo2 = Uuid::new_v4();

        for deployment in [
            Deployment {
                id: Uuid::new_v4(),
                name: "abc".to_string(),
                state: State::Built,
                last_update: Utc.ymd(2022, 4, 25).and_hms(4, 29, 33),
            },
            Deployment {
                id: Uuid::new_v4(),
                name: "foo".to_string(),
                state: State::Running,
                last_update: Utc.ymd(2022, 4, 25).and_hms(4, 29, 44),
            },
            Deployment {
                id: id_bar,
                name: "bar".to_string(),
                state: State::Running,
                last_update: Utc.ymd(2022, 4, 25).and_hms(4, 33, 48),
            },
            Deployment {
                id: Uuid::new_v4(),
                name: "def".to_string(),
                state: State::Crashed,
                last_update: Utc.ymd(2022, 4, 25).and_hms(4, 38, 52),
            },
            Deployment {
                id: id_foo2,
                name: "foo".to_string(),
                state: State::Running,
                last_update: Utc.ymd(2022, 4, 25).and_hms(4, 42, 32),
            },
        ] {
            p.insert_deployment(deployment).await.unwrap();
        }

        let runnable = p.get_all_runnable_deployments().await.unwrap();
        assert_eq!(
            runnable,
            [
                DeploymentRunnable {
                    id: id_bar,
                    name: "bar".to_string(),
                },
                DeploymentRunnable {
                    id: id_foo2,
                    name: "foo".to_string(),
                },
            ]
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn deployment_deletion() {
        let (p, _) = Persistence::new_in_memory().await;

        let deployments = [
            Deployment {
                id: Uuid::new_v4(),
                name: "x".to_string(),
                state: State::Running,
                last_update: Utc::now(),
            },
            Deployment {
                id: Uuid::new_v4(),
                name: "x".to_string(),
                state: State::Running,
                last_update: Utc::now(),
            },
        ];

        for deployment in deployments.iter() {
            p.insert_deployment(deployment.clone()).await.unwrap();
        }

        assert!(!p.get_deployments("x").await.unwrap().is_empty());
        assert_eq!(p.delete_service("x").await.unwrap(), deployments);
        assert!(p.get_deployments("x").await.unwrap().is_empty());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn log_insert() {
        let (p, _) = Persistence::new_in_memory().await;

        let id = Uuid::new_v4();
        let log = Log {
            id,
            timestamp: Utc::now(),
            state: State::Queued,
            level: Level::Info,
            file: Some("queue.rs".to_string()),
            line: Some(12),
            target: "tests::log_insert".to_string(),
            fields: json!({"message": "job queued"}),
        };

        insert_log(&p.pool, log.clone()).await.unwrap();

        let logs = p.get_deployment_logs(&id).await.unwrap();
        assert!(!logs.is_empty(), "there should be one log");

        assert_eq!(logs.first().unwrap(), &log);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn logs_for_deployment() {
        let (p, _) = Persistence::new_in_memory().await;

        let id_a = Uuid::new_v4();
        let log_a1 = Log {
            id: id_a,
            timestamp: Utc::now(),
            state: State::Queued,
            level: Level::Info,
            file: Some("file.rs".to_string()),
            line: Some(5),
            target: "tests::logs_for_deployment".to_string(),
            fields: json!({"message": "job queued"}),
        };
        let log_b = Log {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            state: State::Queued,
            level: Level::Info,
            file: Some("file.rs".to_string()),
            line: Some(5),
            target: "tests::logs_for_deployment".to_string(),
            fields: json!({"message": "job queued"}),
        };
        let log_a2 = Log {
            id: id_a,
            timestamp: Utc::now(),
            state: State::Building,
            level: Level::Warn,
            file: None,
            line: None,
            target: String::new(),
            fields: json!({"message": "unused Result"}),
        };

        insert_log(&p.pool, log_a1.clone()).await.unwrap();
        insert_log(&p.pool, log_b).await.unwrap();
        insert_log(&p.pool, log_a2.clone()).await.unwrap();

        let logs = p.get_deployment_logs(&id_a).await.unwrap();
        assert!(!logs.is_empty(), "there should be three logs");

        assert_eq!(logs, vec![log_a1, log_a2]);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn log_recorder_event() {
        let (p, handle) = Persistence::new_in_memory().await;

        let id = Uuid::new_v4();
        let event = deploy_layer::Log {
            id,
            timestamp: Utc::now(),
            state: State::Queued,
            level: Level::Info,
            file: Some("file.rs".to_string()),
            line: Some(5),
            target: "tests::log_recorder_event".to_string(),
            fields: json!({"message": "job queued"}),
            r#type: deploy_layer::LogType::Event,
        };

        p.record(event);

        // Drop channel and wait for it to finish
        drop(p.log_send);
        assert!(handle.await.is_ok());

        let logs = get_deployment_logs(&p.pool, &id).await.unwrap();

        assert!(!logs.is_empty(), "there should be one log");

        let log = logs.first().unwrap();
        assert_eq!(log.id, id);
        assert_eq!(log.state, State::Queued);
        assert_eq!(log.level, Level::Info);
        assert_eq!(log.file, Some("file.rs".to_string()));
        assert_eq!(log.line, Some(5));
        assert_eq!(log.fields, json!({"message": "job queued"}));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn log_recorder_state() {
        let (p, handle) = Persistence::new_in_memory().await;

        let id = Uuid::new_v4();

        p.insert_deployment(Deployment {
            id,
            name: "z".to_string(),
            state: State::Queued,
            last_update: Utc.ymd(2022, 4, 29).and_hms(2, 39, 39),
        })
        .await
        .unwrap();
        let state = deploy_layer::Log {
            id,
            timestamp: Utc.ymd(2022, 4, 29).and_hms(2, 39, 59),
            state: State::Running,
            level: Level::Info,
            file: None,
            line: None,
            target: String::new(),
            fields: serde_json::Value::Null,
            r#type: deploy_layer::LogType::State,
        };

        p.record(state);

        // Drop channel and wait for it to finish
        drop(p.log_send);
        assert!(handle.await.is_ok());

        let logs = get_deployment_logs(&p.pool, &id).await.unwrap();

        assert!(!logs.is_empty(), "state change should be logged");

        let log = logs.first().unwrap();
        assert_eq!(log.id, id);
        assert_eq!(log.state, State::Running);
        assert_eq!(log.level, Level::Info);
        assert_eq!(log.fields, json!("NEW STATE"));

        assert_eq!(
            get_deployment(&p.pool, &id).await.unwrap().unwrap(),
            Deployment {
                id,
                name: "z".to_string(),
                state: State::Running,
                last_update: Utc.ymd(2022, 4, 29).and_hms(2, 39, 59),
            }
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn deployment_resources() {
        let (p, _) = Persistence::new_in_memory().await;

        let resource1 = Resource {
            name: "foo".to_string(),
            r#type: ResourceType::Database(resource::DatabaseType::Shared(
                resource::database::SharedType::Postgres,
            )),
            data: json!({"username": "root"}),
        };
        let resource2 = Resource {
            name: "foo".to_string(),
            r#type: ResourceType::Database(resource::DatabaseType::AwsRds(
                resource::database::AwsRdsType::MariaDB,
            )),
            data: json!({"uri": "postgres://localhost"}),
        };
        let resource3 = Resource {
            name: "bar".to_string(),
            r#type: ResourceType::Database(resource::DatabaseType::AwsRds(
                resource::database::AwsRdsType::Postgres,
            )),
            data: json!({"username": "admin"}),
        };
        // This makes sure only the last instance of a type is saved (clashes with [resource1])
        let resource4 = Resource {
            name: "foo".to_string(),
            r#type: ResourceType::Database(resource::DatabaseType::Shared(
                resource::database::SharedType::Postgres,
            )),
            data: json!({"username": "foo"}),
        };

        for resource in [&resource1, &resource2, &resource3, &resource4] {
            p.insert_resource(resource).await.unwrap();
        }

        let resources = p.get_service_resources("foo").await.unwrap();

        assert_eq!(resources, vec![resource2, resource4]);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn secrets() {
        let (p, _) = Persistence::new_in_memory().await;

        p.insert_secret("test_service", "key1", "value1")
            .await
            .unwrap();
        p.insert_secret("another_service", "key2", "value2")
            .await
            .unwrap();
        p.insert_secret("test_service", "key3", "value3")
            .await
            .unwrap();
        p.insert_secret("test_service", "key1", "value1_updated")
            .await
            .unwrap();

        let actual: Vec<_> = p
            .get_secrets("test_service")
            .await
            .unwrap()
            .into_iter()
            .map(|mut i| {
                // Reset dates for test
                i.last_update = Default::default();
                i
            })
            .collect();
        let expected = vec![
            Secret {
                name: "test_service".to_string(),
                key: "key1".to_string(),
                value: "value1_updated".to_string(),
                last_update: Default::default(),
            },
            Secret {
                name: "test_service".to_string(),
                key: "key3".to_string(),
                value: "value3".to_string(),
                last_update: Default::default(),
            },
        ];

        assert_eq!(actual, expected);
    }
}
