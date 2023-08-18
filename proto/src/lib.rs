// This clippy is disabled as per this prost comment
// https://github.com/tokio-rs/prost/issues/661#issuecomment-1156606409
#![allow(clippy::derive_partial_eq_without_eq)]

pub mod provisioner {
    use std::fmt::Display;

    use shuttle_common::{
        database::{self, AwsRdsEngine, SharedEngine},
        DatabaseReadyInfo,
    };

    include!("generated/provisioner.rs");

    impl From<DatabaseResponse> for DatabaseReadyInfo {
        fn from(response: DatabaseResponse) -> Self {
            DatabaseReadyInfo::new(
                response.engine,
                response.username,
                response.password,
                response.database_name,
                response.port,
                response.address_private,
                response.address_public,
            )
        }
    }

    impl From<database::Type> for database_request::DbType {
        fn from(db_type: database::Type) -> Self {
            match db_type {
                database::Type::Shared(engine) => {
                    let engine = match engine {
                        SharedEngine::Postgres => shared::Engine::Postgres(String::new()),
                        SharedEngine::MongoDb => shared::Engine::Mongodb(String::new()),
                    };
                    database_request::DbType::Shared(Shared {
                        engine: Some(engine),
                    })
                }
                database::Type::AwsRds(engine) => {
                    let config = RdsConfig {};
                    let engine = match engine {
                        AwsRdsEngine::Postgres => aws_rds::Engine::Postgres(config),
                        AwsRdsEngine::MariaDB => aws_rds::Engine::Mariadb(config),
                        AwsRdsEngine::MySql => aws_rds::Engine::Mysql(config),
                    };
                    database_request::DbType::AwsRds(AwsRds {
                        engine: Some(engine),
                    })
                }
            }
        }
    }

    impl From<database_request::DbType> for Option<database::Type> {
        fn from(db_type: database_request::DbType) -> Self {
            match db_type {
                database_request::DbType::Shared(Shared {
                    engine: Some(engine),
                }) => match engine {
                    shared::Engine::Postgres(_) => {
                        Some(database::Type::Shared(SharedEngine::Postgres))
                    }
                    shared::Engine::Mongodb(_) => {
                        Some(database::Type::Shared(SharedEngine::MongoDb))
                    }
                },
                database_request::DbType::AwsRds(AwsRds {
                    engine: Some(engine),
                }) => match engine {
                    aws_rds::Engine::Postgres(_) => {
                        Some(database::Type::AwsRds(AwsRdsEngine::Postgres))
                    }
                    aws_rds::Engine::Mysql(_) => Some(database::Type::AwsRds(AwsRdsEngine::MySql)),
                    aws_rds::Engine::Mariadb(_) => {
                        Some(database::Type::AwsRds(AwsRdsEngine::MariaDB))
                    }
                },
                database_request::DbType::Shared(Shared { engine: None })
                | database_request::DbType::AwsRds(AwsRds { engine: None }) => None,
            }
        }
    }

    impl Display for aws_rds::Engine {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Self::Mariadb(_) => write!(f, "mariadb"),
                Self::Mysql(_) => write!(f, "mysql"),
                Self::Postgres(_) => write!(f, "postgres"),
            }
        }
    }
}

pub mod runtime {
    use std::{path::PathBuf, time::Duration};

    use anyhow::Context;
    use shuttle_common::claims::{
        ClaimLayer, ClaimService, InjectPropagation, InjectPropagationLayer,
    };
    use tokio::process;
    use tonic::transport::{Channel, Endpoint};
    use tower::ServiceBuilder;
    use tracing::{info, trace};

    pub enum StorageManagerType {
        Artifacts(PathBuf),
        WorkingDir(PathBuf),
    }

    include!("generated/runtime.rs");

    pub async fn start(
        wasm: bool,
        storage_manager_type: StorageManagerType,
        provisioner_address: &str,
        logger_uri: &str,
        auth_uri: Option<&String>,
        port: u16,
        get_runtime_executable: impl FnOnce() -> PathBuf,
    ) -> anyhow::Result<(
        process::Child,
        runtime_client::RuntimeClient<ClaimService<InjectPropagation<Channel>>>,
    )> {
        let (storage_manager_type, storage_manager_path) = match storage_manager_type {
            StorageManagerType::Artifacts(path) => ("artifacts", path),
            StorageManagerType::WorkingDir(path) => ("working-dir", path),
        };

        let port = &port.to_string();
        let storage_manager_path = &storage_manager_path.display().to_string();
        let runtime_executable_path = get_runtime_executable();

        let args = if wasm {
            vec!["--port", port]
        } else {
            let mut args = vec![
                "--port",
                port,
                "--provisioner-address",
                provisioner_address,
                "--logger-uri",
                logger_uri,
                "--storage-manager-type",
                storage_manager_type,
                "--storage-manager-path",
                storage_manager_path,
            ];

            if let Some(auth_uri) = auth_uri {
                args.append(&mut vec!["--auth-uri", auth_uri]);
            }

            args
        };

        trace!(
            "Spawning runtime process {:?} {:?}",
            runtime_executable_path,
            args
        );
        let runtime = process::Command::new(runtime_executable_path)
            .args(&args)
            .kill_on_drop(true)
            .spawn()
            .context("spawning runtime process")?;

        info!("connecting runtime client");
        let conn = Endpoint::new(format!("http://127.0.0.1:{port}"))
            .context("creating runtime client endpoint")?
            .connect_timeout(Duration::from_secs(5));

        // Wait for the spawned process to open the endpoint port.
        // Connecting instantly does not give it enough time.
        let channel = tokio::time::timeout(Duration::from_millis(7000), async move {
            let mut ms = 5;
            loop {
                if let Ok(channel) = conn.connect().await {
                    break channel;
                }
                trace!("waiting for runtime endpoint to open");
                // exponential backoff
                tokio::time::sleep(Duration::from_millis(ms)).await;
                ms *= 2;
            }
        })
        .await
        .context("runtime client endpoint did not open in time")?;

        let channel = ServiceBuilder::new()
            .layer(ClaimLayer)
            .layer(InjectPropagationLayer)
            .service(channel);
        let runtime_client = runtime_client::RuntimeClient::new(channel);

        Ok((runtime, runtime_client))
    }
}

pub mod resource_recorder {
    use std::str::FromStr;

    include!("generated/resource_recorder.rs");

    impl From<record_request::Resource> for shuttle_common::resource::Response {
        fn from(resource: record_request::Resource) -> Self {
            shuttle_common::resource::Response {
                r#type: shuttle_common::resource::Type::from_str(resource.r#type.as_str())
                    .expect("to have a valid resource string"),
                config: serde_json::from_slice(&resource.config)
                    .expect("to have JSON valid config"),
                data: serde_json::from_slice(&resource.data).expect("to have JSON valid data"),
            }
        }
    }

    impl From<Resource> for shuttle_common::resource::Response {
        fn from(resource: Resource) -> Self {
            shuttle_common::resource::Response {
                r#type: shuttle_common::resource::Type::from_str(resource.r#type.as_str())
                    .expect("to have a valid resource string"),
                config: serde_json::from_slice(&resource.config)
                    .expect("to have JSON valid config"),
                data: serde_json::from_slice(&resource.data).expect("to have JSON valid data"),
            }
        }
    }
}

pub mod logger {
    use chrono::{DateTime, NaiveDateTime, Utc};
    use shuttle_common::tracing::{FILEPATH_KEY, LINENO_KEY, TARGET_KEY};
    use tracing::error;

    include!("generated/logger.rs");

    impl From<LogLevel> for shuttle_common::log::Level {
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

    impl From<LogItem> for shuttle_common::LogItem {
        fn from(value: LogItem) -> Self {
            let proto_timestamp = value.timestamp.clone().unwrap_or_default();
            let level = value.level();
            let mut fields: serde_json::Map<String, serde_json::Value> =
                match serde_json::from_slice(&value.fields) {
                    Ok(serde_json::Value::Object(o)) => o,
                    Ok(_) => {
                        error!("unexpected JSON value, expected an object");
                        serde_json::Map::new()
                    }
                    Err(err) => {
                        error!("malformed fields object: {err}");
                        serde_json::Map::new()
                    }
                };

            // Safe to unwrap since we've previously serialised the fields we're removing below.
            let file = fields
                .remove(FILEPATH_KEY)
                .map(|v| v.as_str().unwrap_or_default().to_string());
            let line = fields
                .remove(LINENO_KEY)
                .map(|v| u32::try_from(v.as_u64().unwrap_or_default()).unwrap_or_default());
            let target = fields
                .remove(TARGET_KEY)
                .map(|v| v.as_str().unwrap_or_default().to_string())
                .unwrap_or_default();

            Self {
                id: Default::default(),
                timestamp: DateTime::from_utc(
                    NaiveDateTime::from_timestamp_opt(
                        proto_timestamp.seconds,
                        proto_timestamp.nanos.try_into().unwrap_or_default(),
                    )
                    .unwrap_or_default(),
                    Utc,
                ),
                // TODO: update this to the corresponding state shown in the runtime log, when present
                state: shuttle_common::deployment::State::Running,
                level: level.into(),
                file,
                line,
                target,
                fields: serde_json::to_vec(&fields).unwrap_or_default(),
            }
        }
    }
}
