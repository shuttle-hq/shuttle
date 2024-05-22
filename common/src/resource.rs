use std::{fmt::Display, str::FromStr};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use crate::{constants::RESOURCE_SCHEMA_VERSION, database};

/// Return this struct as a resource config to make Shuttle provision it
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProvisionResourceRequest {
    /// The version of the config+data schema for this Shuttle resource
    pub version: u32,
    /// The type of this resource
    pub r#type: Type,
    /// The config used when creating this resource.
    /// Use `Self::version` and `Self::r#type` to know how to parse this data.
    pub config: Value,

    /// Arbitrary extra data to include in this resource
    pub custom: Value,
}

impl ProvisionResourceRequest {
    pub fn new(r#type: Type, config: Value, custom: Value) -> Self {
        Self {
            version: RESOURCE_SCHEMA_VERSION,
            r#type,
            config,
            custom,
        }
    }
}

/// Helper for deserializing
#[derive(Deserialize)]
#[serde(untagged)] // Try deserializing as a Shuttle resource, fall back to a custom value
pub enum ResourceInput {
    Shuttle(ProvisionResourceRequest),
    Custom(Value),
}

/// Returned when provisioning a Shuttle resource
#[derive(Serialize, Deserialize)]
pub struct ShuttleResourceOutput<T> {
    /// The output type for this Shuttle resource,
    /// contains the data from the provisioner response
    pub output: T,

    /// Arbitrary extra data in this resource
    pub custom: Value,
}

/// Common type to hold all the information we need for a generic resource
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct Response {
    /// The type of this resource.
    pub r#type: Type,

    /// The config used when creating this resource. Use the `r#type` to know how to parse this data.
    pub config: Value,

    /// The data associated with this resource. Use the `r#type` to know how to parse this data.
    pub data: Value,
}

impl Response {
    pub fn into_bytes(self) -> Vec<u8> {
        self.to_bytes()
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).expect("to turn resource into a vec")
    }

    pub fn from_bytes(bytes: Vec<u8>) -> Self {
        serde_json::from_slice(&bytes).expect("to turn bytes into a resource")
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Type {
    Database(database::Type),
    Secrets,
    Persist,
    /// Local provisioner only
    Container,
}

#[derive(Debug, Error)]
pub enum InvalidResourceType {
    #[error("'{0}' is an unknown database type")]
    Type(String),

    #[error("{0}")]
    Database(String),
}

impl FromStr for Type {
    type Err = InvalidResourceType;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Some((prefix, rest)) = s.split_once("::") {
            match prefix {
                "database" => Ok(Self::Database(
                    database::Type::from_str(rest).map_err(InvalidResourceType::Database)?,
                )),
                _ => Err(InvalidResourceType::Type(prefix.to_string())),
            }
        } else {
            match s {
                "secrets" => Ok(Self::Secrets),
                "persist" => Ok(Self::Persist),
                "container" => Ok(Self::Container),
                _ => Err(InvalidResourceType::Type(s.to_string())),
            }
        }
    }
}

impl Display for Type {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Type::Database(db_type) => write!(f, "database::{db_type}"),
            Type::Secrets => write!(f, "secrets"),
            Type::Persist => write!(f, "persist"),
            Type::Container => write!(f, "container"),
        }
    }
}

// this can be removed when deployers AND r-r no longer hold resources in sqlite state
#[cfg(feature = "sqlx")]
mod _sqlx {
    use std::{borrow::Cow, str::FromStr};

    use sqlx::{
        sqlite::{SqliteArgumentValue, SqliteValueRef},
        Database, Sqlite,
    };

    use super::Type;

    impl<DB: Database> sqlx::Type<DB> for Type
    where
        str: sqlx::Type<DB>,
    {
        fn type_info() -> <DB as Database>::TypeInfo {
            <str as sqlx::Type<DB>>::type_info()
        }
    }

    impl<'q> sqlx::Encode<'q, Sqlite> for Type {
        fn encode_by_ref(&self, args: &mut Vec<SqliteArgumentValue<'q>>) -> sqlx::encode::IsNull {
            args.push(SqliteArgumentValue::Text(Cow::Owned(self.to_string())));

            sqlx::encode::IsNull::No
        }
    }

    impl<'r> sqlx::Decode<'r, Sqlite> for Type {
        fn decode(value: SqliteValueRef<'r>) -> Result<Self, sqlx::error::BoxDynError> {
            let value = <&str as sqlx::Decode<Sqlite>>::decode(value)?;

            Self::from_str(value).map_err(Into::into)
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn to_string_and_back() {
        let inputs = [
            Type::Database(database::Type::AwsRds(database::AwsRdsEngine::Postgres)),
            Type::Database(database::Type::AwsRds(database::AwsRdsEngine::MySql)),
            Type::Database(database::Type::AwsRds(database::AwsRdsEngine::MariaDB)),
            Type::Database(database::Type::Shared(database::SharedEngine::Postgres)),
            Type::Database(database::Type::Shared(database::SharedEngine::MongoDb)),
            Type::Secrets,
            Type::Persist,
            Type::Container,
        ];

        for input in inputs {
            let actual = Type::from_str(&input.to_string()).unwrap();
            assert_eq!(input, actual, ":{} should map back to itself", input);
        }
    }
}
