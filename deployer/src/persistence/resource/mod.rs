pub mod database;

use crate::error::Result;
use sqlx::{
    sqlite::{SqliteArgumentValue, SqliteValueRef},
    Database, Sqlite,
};
use std::{borrow::Cow, fmt::Display, str::FromStr};

pub use self::database::Type as DatabaseType;

#[async_trait::async_trait]
pub trait ResourceRecorder: Clone + Send + Sync + 'static {
    async fn insert_resource(&self, resource: &Resource) -> Result<()>;
}

#[derive(sqlx::FromRow, Debug, Eq, PartialEq)]
pub struct Resource {
    pub name: String,
    pub r#type: Type,
    pub data: serde_json::Value,
}

impl From<Resource> for shuttle_common::resource::Response {
    fn from(resource: Resource) -> Self {
        shuttle_common::resource::Response {
            service_name: resource.name,
            r#type: resource.r#type.into(),
            data: resource.data,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Type {
    Database(DatabaseType),
}

impl From<Type> for shuttle_common::resource::Type {
    fn from(r#type: Type) -> Self {
        match r#type {
            Type::Database(r#type) => Self::Database(r#type.into()),
        }
    }
}

impl Display for Type {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Type::Database(db_type) => write!(f, "database::{db_type}"),
        }
    }
}

impl FromStr for Type {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        if let Some((prefix, rest)) = s.split_once("::") {
            match prefix {
                "database" => Ok(Self::Database(DatabaseType::from_str(rest)?)),
                _ => Err(format!("'{prefix}' is an unknown resource type")),
            }
        } else {
            Err(format!("'{s}' is an unknown resource type"))
        }
    }
}

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
    fn decode(value: SqliteValueRef<'r>) -> std::result::Result<Self, sqlx::error::BoxDynError> {
        let value = <&str as sqlx::Decode<Sqlite>>::decode(value)?;

        Self::from_str(value).map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::{database, Type};

    #[test]
    fn to_string_and_back() {
        let inputs = [
            Type::Database(database::Type::AwsRds(database::AwsRdsType::Postgres)),
            Type::Database(database::Type::AwsRds(database::AwsRdsType::MySql)),
            Type::Database(database::Type::AwsRds(database::AwsRdsType::MariaDB)),
            Type::Database(database::Type::Shared),
        ];

        for input in inputs {
            let actual = Type::from_str(&input.to_string()).unwrap();
            assert_eq!(input, actual, ":{} should map back to itself", input);
        }
    }
}
