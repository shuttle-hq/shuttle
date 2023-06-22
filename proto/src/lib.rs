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
    use std::{
        convert::TryFrom,
        path::PathBuf,
        time::{Duration, SystemTime},
    };

    use anyhow::Context;
    use chrono::DateTime;
    use prost_types::Timestamp;
    use shuttle_common::{
        claims::{ClaimLayer, ClaimService, InjectPropagation, InjectPropagationLayer},
        ParseError,
    };
    use tokio::process;
    use tonic::transport::{Channel, Endpoint};
    use tower::ServiceBuilder;
    use tracing::info;

    pub enum StorageManagerType {
        Artifacts(PathBuf),
        WorkingDir(PathBuf),
    }

    include!("generated/runtime.rs");

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

    impl TryFrom<LogItem> for shuttle_common::LogItem {
        type Error = ParseError;

        fn try_from(log: LogItem) -> Result<Self, Self::Error> {
            Ok(Self {
                id: Default::default(),
                timestamp: DateTime::from(SystemTime::try_from(log.timestamp.unwrap_or_default())?),
                state: shuttle_common::deployment::State::Running,
                level: LogLevel::from_i32(log.level).unwrap_or_default().into(),
                file: log.file,
                line: log.line,
                target: log.target,
                fields: log.fields,
            })
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
                timestamp: Some(Timestamp::from(SystemTime::from(log.timestamp))),
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

    impl From<&tracing::Level> for LogLevel {
        fn from(level: &tracing::Level) -> Self {
            match *level {
                tracing::Level::TRACE => Self::Trace,
                tracing::Level::DEBUG => Self::Debug,
                tracing::Level::INFO => Self::Info,
                tracing::Level::WARN => Self::Warn,
                tracing::Level::ERROR => Self::Error,
            }
        }
    }

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

        let runtime = process::Command::new(runtime_executable_path)
            .args(&args)
            .kill_on_drop(true)
            .spawn()
            .context("spawning runtime process")?;

        // Sleep because the timeout below does not seem to work
        // TODO: investigate why
        tokio::time::sleep(Duration::from_secs(2)).await;

        info!("connecting runtime client");
        let conn = Endpoint::new(format!("http://127.0.0.1:{port}"))
            .context("creating runtime client endpoint")?
            .connect_timeout(Duration::from_secs(5));

        let channel = conn.connect().await.context("connecting runtime client")?;
        let channel = ServiceBuilder::new()
            .layer(ClaimLayer)
            .layer(InjectPropagationLayer)
            .service(channel);
        let runtime_client = runtime_client::RuntimeClient::new(channel);

        Ok((runtime, runtime_client))
    }
}

pub mod resource_recorder {
    include!("generated/resource_recorder.rs");
}

pub mod auth {
    use std::sync::Arc;
    use std::time::Duration;

    use anyhow::Context;
    use async_trait::async_trait;
    use shuttle_common::backends::auth::{PublicKeyFn, PublicKeyFnError, PUBLIC_KEY_CACHE_KEY};
    use shuttle_common::backends::cache::{CacheManagement, CacheManager};
    use shuttle_common::claims::InjectPropagation;
    use shuttle_common::claims::InjectPropagationLayer;
    use shuttle_common::models::user::Response;
    use tonic::transport::Channel;
    use tonic::transport::Endpoint;
    use tonic::transport::Uri;
    use tonic::Request;
    use tower::ServiceBuilder;
    use tracing::trace;

    use self::auth_client::AuthClient;

    include!("generated/auth.rs");

    impl From<UserResponse> for Response {
        fn from(value: UserResponse) -> Self {
            Response {
                name: value.account_name,
                key: value.key,
                account_tier: value.account_tier,
            }
        }
    }

    pub async fn client(auth_uri: &Uri) -> anyhow::Result<AuthClient<InjectPropagation<Channel>>> {
        let conn = Endpoint::new(auth_uri.clone())
            .context("auth uri should be valid endpoint")?
            .connect_timeout(Duration::from_secs(5));

        let channel = conn
            .connect()
            .await
            .context("auth service should be reachable")?;

        let channel = ServiceBuilder::new()
            .layer(InjectPropagationLayer)
            .service(channel);

        Ok(AuthClient::new(channel))
    }

    #[derive(Clone)]
    pub struct AuthPublicKey {
        auth_client: AuthClient<InjectPropagation<Channel>>,
        cache_manager: Arc<Box<dyn CacheManagement<Value = Vec<u8>>>>,
    }

    impl AuthPublicKey {
        pub fn new(auth_client: AuthClient<InjectPropagation<Channel>>) -> Self {
            let public_key_cache_manager = CacheManager::new(1);
            Self {
                auth_client,
                cache_manager: Arc::new(Box::new(public_key_cache_manager)),
            }
        }
    }

    #[async_trait]
    impl PublicKeyFn for AuthPublicKey {
        type Error = PublicKeyFnError;

        async fn public_key(&self) -> Result<Vec<u8>, Self::Error> {
            if let Some(public_key) = self.cache_manager.get(PUBLIC_KEY_CACHE_KEY) {
                trace!("found public key in the cache, returning it");

                Ok(public_key)
            } else {
                let request = Request::new(PublicKeyRequest::default());

                let mut client = self.auth_client.clone();

                let response = client
                    .public_key(request)
                    .await
                    .context("failed to retrieve public key from auth service")?
                    .into_inner();

                trace!("inserting public key from auth service into cache");
                self.cache_manager.insert(
                    PUBLIC_KEY_CACHE_KEY,
                    response.public_key.clone(),
                    std::time::Duration::from_secs(60),
                );

                Ok(response.public_key)
            }
        }
    }
}

pub mod builder {
    include!("generated/builder.rs");
}

pub mod logger {
    include!("generated/logger.rs");
}
