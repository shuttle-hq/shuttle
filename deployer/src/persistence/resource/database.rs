use std::{fmt::Display, str::FromStr};

use strum::{Display, EnumString};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Type {
    AwsRds(AwsRdsType),
    Shared(SharedType),
    Filesystem,
}

#[derive(Clone, Copy, Debug, Display, EnumString, Eq, PartialEq)]
#[strum(serialize_all = "lowercase")]
pub enum AwsRdsType {
    Postgres,
    MySql,
    MariaDB,
}

#[derive(Clone, Copy, Debug, Display, EnumString, Eq, PartialEq)]
#[strum(serialize_all = "lowercase")]
pub enum SharedType {
    Postgres,
    MongoDb,
}

impl From<Type> for shuttle_common::database::Type {
    fn from(r#type: Type) -> Self {
        match r#type {
            Type::AwsRds(rds_type) => Self::AwsRds(rds_type.into()),
            Type::Shared(shared_type) => Self::Shared(shared_type.into()),
            Type::Filesystem => Self::Filesystem,
        }
    }
}

impl From<AwsRdsType> for shuttle_common::database::AwsRdsEngine {
    fn from(rds_type: AwsRdsType) -> Self {
        match rds_type {
            AwsRdsType::Postgres => Self::Postgres,
            AwsRdsType::MySql => Self::MySql,
            AwsRdsType::MariaDB => Self::MariaDB,
        }
    }
}

impl From<SharedType> for shuttle_common::database::SharedEngine {
    fn from(shared_type: SharedType) -> Self {
        match shared_type {
            SharedType::Postgres => Self::Postgres,
            SharedType::MongoDb => Self::MongoDb,
        }
    }
}

impl From<shuttle_common::database::Type> for Type {
    fn from(r#type: shuttle_common::database::Type) -> Self {
        match r#type {
            shuttle_common::database::Type::AwsRds(rds_type) => Self::AwsRds(rds_type.into()),
            shuttle_common::database::Type::Shared(shared_type) => Self::Shared(shared_type.into()),
            shuttle_common::database::Type::Filesystem => Self::Filesystem,
        }
    }
}

impl From<shuttle_common::database::AwsRdsEngine> for AwsRdsType {
    fn from(rds_type: shuttle_common::database::AwsRdsEngine) -> Self {
        match rds_type {
            shuttle_common::database::AwsRdsEngine::Postgres => Self::Postgres,
            shuttle_common::database::AwsRdsEngine::MySql => Self::MySql,
            shuttle_common::database::AwsRdsEngine::MariaDB => Self::MariaDB,
        }
    }
}

impl From<shuttle_common::database::SharedEngine> for SharedType {
    fn from(shared_type: shuttle_common::database::SharedEngine) -> Self {
        match shared_type {
            shuttle_common::database::SharedEngine::Postgres => Self::Postgres,
            shuttle_common::database::SharedEngine::MongoDb => Self::MongoDb,
        }
    }
}

impl Display for Type {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Type::AwsRds(rds_type) => write!(f, "aws_rds::{rds_type}"),
            Type::Shared(shared_type) => write!(f, "shared::{shared_type}"),
            Type::Filesystem => write!(f, "filesystem"),
        }
    }
}

impl FromStr for Type {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        if let Some((prefix, rest)) = s.split_once("::") {
            match prefix {
                "aws_rds" => Ok(Self::AwsRds(
                    AwsRdsType::from_str(rest).map_err(|e| e.to_string())?,
                )),
                "shared" => Ok(Self::Shared(
                    SharedType::from_str(rest).map_err(|e| e.to_string())?,
                )),
                _ => Err(format!("'{prefix}' is an unknown database type")),
            }
        } else {
            Err(format!("'{s}' is an unknown database type"))
        }
    }
}
