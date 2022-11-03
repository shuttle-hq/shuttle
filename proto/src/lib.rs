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
    use std::time::SystemTime;

    use prost_types::Timestamp;

    tonic::include_proto!("runtime");

    impl From<shuttle_common::LogItem> for LogItem {
        fn from(log: shuttle_common::LogItem) -> Self {
            Self {
                id: log.id.to_string(),
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
}
