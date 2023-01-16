// This clippy is disabled as per this prost comment
// https://github.com/tokio-rs/prost/issues/661#issuecomment-1156606409
#![allow(clippy::derive_partial_eq_without_eq)]

pub mod provisioner {
    use std::fmt::Display;

    use shuttle_common::{
        database::{self, AwsRdsEngine, SharedEngine},
        DatabaseReadyInfo,
    };

    tonic::include_proto!("provisioner");

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
    use std::{
        path::PathBuf,
        process::Command,
        time::{Duration, SystemTime},
    };

    use anyhow::Context;
    use chrono::DateTime;
    use prost_types::Timestamp;
    use tokio::process;
    use tonic::transport::{Channel, Endpoint};
    use tracing::info;
    use uuid::Uuid;

    pub enum StorageManagerType {
        Artifacts(PathBuf),
        WorkingDir(PathBuf),
    }

    tonic::include_proto!("runtime");

    impl From<shuttle_common::LogItem> for LogItem {
        fn from(log: shuttle_common::LogItem) -> Self {
            Self {
                id: log.id.into_bytes().to_vec(),
                timestamp: Some(Timestamp::from(SystemTime::from(log.timestamp))),
                state: LogState::from(log.state) as i32,
                level: LogLevel::from(log.level) as i32,
                file: log.file,
                line: log.line,
                target: log.target,
                fields: log.fields,
            }
        }
    }

    impl From<shuttle_common::deployment::State> for LogState {
        fn from(state: shuttle_common::deployment::State) -> Self {
            match state {
                shuttle_common::deployment::State::Queued => Self::Queued,
                shuttle_common::deployment::State::Building => Self::Building,
                shuttle_common::deployment::State::Built => Self::Built,
                shuttle_common::deployment::State::Loading => Self::Loading,
                shuttle_common::deployment::State::Running => Self::Running,
                shuttle_common::deployment::State::Completed => Self::Completed,
                shuttle_common::deployment::State::Stopped => Self::Stopped,
                shuttle_common::deployment::State::Crashed => Self::Crashed,
                shuttle_common::deployment::State::Unknown => Self::Unknown,
            }
        }
    }

    impl From<shuttle_common::log::Level> for LogLevel {
        fn from(level: shuttle_common::log::Level) -> Self {
            match level {
                shuttle_common::log::Level::Trace => Self::Trace,
                shuttle_common::log::Level::Debug => Self::Debug,
                shuttle_common::log::Level::Info => Self::Info,
                shuttle_common::log::Level::Warn => Self::Warn,
                shuttle_common::log::Level::Error => Self::Error,
            }
        }
    }

    impl From<LogItem> for shuttle_common::LogItem {
        fn from(log: LogItem) -> Self {
            Self {
                id: Uuid::from_slice(&log.id).unwrap(),
                timestamp: DateTime::from(SystemTime::try_from(log.timestamp.unwrap()).unwrap()),
                state: LogState::from_i32(log.state).unwrap().into(),
                level: LogLevel::from_i32(log.level).unwrap().into(),
                file: log.file,
                line: log.line,
                target: log.target,
                fields: log.fields,
            }
        }
    }

    impl From<LogState> for shuttle_common::deployment::State {
        fn from(state: LogState) -> Self {
            match state {
                LogState::Queued => Self::Queued,
                LogState::Building => Self::Building,
                LogState::Built => Self::Built,
                LogState::Loading => Self::Loading,
                LogState::Running => Self::Running,
                LogState::Completed => Self::Completed,
                LogState::Stopped => Self::Stopped,
                LogState::Crashed => Self::Crashed,
                LogState::Unknown => Self::Unknown,
            }
        }
    }

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

    impl From<shuttle_common::wasm::Log> for LogItem {
        fn from(log: shuttle_common::wasm::Log) -> Self {
            let file = if log.file.is_empty() {
                None
            } else {
                Some(log.file)
            };

            let line = if log.line == 0 { None } else { Some(log.line) };

            Self {
                id: Default::default(),
                timestamp: Some(Timestamp::from(SystemTime::from(log.timestamp))),
                state: LogState::Running as i32,
                level: LogLevel::from(log.level) as i32,
                file,
                line,
                target: log.target,
                fields: log.fields,
            }
        }
    }

    impl From<shuttle_common::wasm::Level> for LogLevel {
        fn from(level: shuttle_common::wasm::Level) -> Self {
            match level {
                shuttle_common::wasm::Level::Trace => Self::Trace,
                shuttle_common::wasm::Level::Debug => Self::Debug,
                shuttle_common::wasm::Level::Info => Self::Info,
                shuttle_common::wasm::Level::Warn => Self::Warn,
                shuttle_common::wasm::Level::Error => Self::Error,
            }
        }
    }

    pub async fn start(
        wasm: bool,
        storage_manager_type: StorageManagerType,
        provisioner_address: &str,
        port: u16,
    ) -> anyhow::Result<(process::Child, runtime_client::RuntimeClient<Channel>)> {
        let runtime_flag = if wasm { "--axum" } else { "--legacy" };

        let (storage_manager_type, storage_manager_path) = match storage_manager_type {
            StorageManagerType::Artifacts(path) => ("artifacts", path),
            StorageManagerType::WorkingDir(path) => ("working-dir", path),
        };

        let runtime_executable = get_runtime_executable();

        let runtime = process::Command::new(runtime_executable)
            .args([
                runtime_flag,
                "--port",
                &port.to_string(),
                "--provisioner-address",
                provisioner_address,
                "--storage-manager-type",
                storage_manager_type,
                "--storage-manager-path",
                &storage_manager_path.display().to_string(),
            ])
            .spawn()
            .context("spawning runtime process")?;

        // Sleep because the timeout below does not seem to work
        // TODO: investigate why
        tokio::time::sleep(Duration::from_secs(2)).await;

        info!("connecting runtime client");
        let conn = Endpoint::new(format!("http://127.0.0.1:{port}"))
            .context("creating runtime client endpoint")?
            .connect_timeout(Duration::from_secs(5));

        let runtime_client = runtime_client::RuntimeClient::connect(conn)
            .await
            .context("connecting runtime client")?;

        Ok((runtime, runtime_client))
    }

    fn get_runtime_executable() -> PathBuf {
        // When this library is compiled in debug mode with `cargo run --bin cargo-shuttle`,
        // install the checked-out local version of `shuttle-runtime
        if cfg!(debug_assertions) {
            // Path to cargo-shuttle
            let manifest_dir = env!("CARGO_MANIFEST_DIR");

            // Canonicalized path to shuttle-runtime
            let path = std::fs::canonicalize(format!("{manifest_dir}/../runtime"))
                .expect("failed to canonicalize path to runtime");

            Command::new("cargo")
                .arg("install")
                .arg("shuttle-runtime")
                .arg("--path")
                .arg(path)
                .output()
                .expect("failed to install the shuttle runtime");
        // When this library is compiled in release mode with `cargo install cargo-shuttle`,
        // install the latest released `shuttle-runtime`
        } else {
            Command::new("cargo")
                .arg("install")
                .arg("shuttle-runtime")
                .arg("--git")
                .arg("https://github.com/shuttle-hq/shuttle")
                .arg("--branch")
                .arg("production")
                .output()
                .expect("failed to install the shuttle runtime");
        }

        let cargo_home = home::cargo_home().expect("failed to find cargo home directory");

        cargo_home.join("bin/shuttle-runtime")
    }
}
