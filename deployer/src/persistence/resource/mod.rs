pub mod database;

use sqlx::{
    sqlite::{SqliteArgumentValue, SqliteValueRef},
    Database, Sqlite,
};
use std::{borrow::Cow, fmt::Display, str::FromStr};
use uuid::Uuid;

pub use self::database::Type as DatabaseType;

/// Types that can record and retrieve resource allocations
#[async_trait::async_trait]
pub trait ResourceManager: Clone + Send + Sync + 'static {
    type Err: std::error::Error;

    async fn insert_resource(&self, resource: &Resource) -> Result<(), Self::Err>;
    async fn get_resources(&self, service_id: &Uuid) -> Result<Vec<Resource>, Self::Err>;
}

#[derive(sqlx::FromRow, Debug, Eq, PartialEq)]
pub struct Resource {
    pub service_id: Uuid,
    pub r#type: Type,
    pub data: serde_json::Value,
    pub config: serde_json::Value,
}

impl From<Resource> for shuttle_common::resource::Response {
    fn from(resource: Resource) -> Self {
        shuttle_common::resource::Response {
            r#type: resource.r#type.into(),
            config: resource.config,
            data: resource.data,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Type {
    Database(DatabaseType),
    Secrets,
    StaticFolder,
    Persist,
    DynamoDB
}

impl From<Type> for shuttle_common::resource::Type {
    fn from(r#type: Type) -> Self {
        match r#type {
            Type::Database(r#type) => Self::Database(r#type.into()),
            Type::Secrets => Self::Secrets,
            Type::StaticFolder => Self::StaticFolder,
            Type::Persist => Self::Persist,
            Type::DynamoDB => Self::DynamoDB
        }
    }
}

impl From<shuttle_common::resource::Type> for Type {
    fn from(r#type: shuttle_common::resource::Type) -> Self {
        match r#type {
            shuttle_common::resource::Type::Database(r#type) => Self::Database(r#type.into()),
            shuttle_common::resource::Type::Secrets => Self::Secrets,
            shuttle_common::resource::Type::StaticFolder => Self::StaticFolder,
            shuttle_common::resource::Type::Persist => Self::Persist,
            shuttle_common::resource::Type::DynamoDB => Self::DynamoDB
        }
    }
}

impl Display for Type {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Type::Database(db_type) => write!(f, "database::{db_type}"),
            Type::Secrets => write!(f, "secrets"),
            Type::StaticFolder => write!(f, "static_folder"),
            Type::Persist => write!(f, "persist"),
            Type::DynamoDB => write!(f, "dynamodb")
        }
    }
}

impl FromStr for Type {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Some((prefix, rest)) = s.split_once("::") {
            match prefix {
                "database" => Ok(Self::Database(DatabaseType::from_str(rest)?)),
                _ => Err(format!("'{prefix}' is an unknown resource type")),
            }
        } else {
            match s {
                "secrets" => Ok(Self::Secrets),
                "static_folder" => Ok(Self::StaticFolder),
                "persist" => Ok(Self::Persist),
                "dynamodb" => Ok(Self::DynamoDB),
                _ => Err(format!("'{s}' is an unknown resource type")),
            }
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
    fn decode(value: SqliteValueRef<'r>) -> Result<Self, sqlx::error::BoxDynError> {
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
            Type::Database(database::Type::Shared(database::SharedType::Postgres)),
            Type::Database(database::Type::Shared(database::SharedType::MongoDb)),
            Type::Secrets,
            Type::StaticFolder,
            Type::Persist,
            Type::DynamoDB
        ];

        for input in inputs {
            let actual = Type::from_str(&input.to_string()).unwrap();
            assert_eq!(input, actual, ":{} should map back to itself", input);
        }
    }
}
