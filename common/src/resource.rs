use std::{fmt::Display, str::FromStr};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::database;

/// Common type to hold all the information we need for a generic resource
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct Response {
    /// The type of this resource.
    pub r#type: Type,

    /// The config used when creating this resource. Use the [Self::r#type] to know how to parse this data.
    pub config: Value,

    /// The data associated with this resource. Use the [Self::r#type] to know how to parse this data.
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
    Custom,
}

impl FromStr for Type {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Some((prefix, rest)) = s.split_once("::") {
            match prefix {
                "database" => Ok(Self::Database(database::Type::from_str(rest)?)),
                _ => Err(format!("'{prefix}' is an unknown resource type")),
            }
        } else {
            match s {
                "secrets" => Ok(Self::Secrets),
                "persist" => Ok(Self::Persist),
                "custom" => Ok(Self::Custom),
                _ => Err(format!("'{s}' is an unknown resource type")),
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
            Type::Custom => write!(f, "custom"),
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
            Type::Custom,
        ];

        for input in inputs {
            let actual = Type::from_str(&input.to_string()).unwrap();
            assert_eq!(input, actual, ":{} should map back to itself", input);
        }
    }
}
