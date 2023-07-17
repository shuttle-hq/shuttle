use std::{borrow::Cow, fmt::Display, str::FromStr};

use sqlx::Database;

pub mod database;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Type {
    Database(database::Type),
    Secrets,
    StaticFolder,
    Persist,
    Turso,
    Custom,
}

impl Display for Type {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Type::Database(db_type) => write!(f, "database::{db_type}"),
            Type::Secrets => write!(f, "secrets"),
            Type::StaticFolder => write!(f, "static_folder"),
            Type::Persist => write!(f, "persist"),
            Type::Turso => write!(f, "turso"),
            Type::Custom => write!(f, "custom"),
        }
    }
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
                "static_folder" => Ok(Self::StaticFolder),
                "persist" => Ok(Self::Persist),
                "turso" => Ok(Self::Turso),
                "custom" => Ok(Self::Custom),
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

impl<'q> sqlx::Encode<'q, sqlx::Sqlite> for Type {
    fn encode_by_ref(
        &self,
        args: &mut Vec<sqlx::sqlite::SqliteArgumentValue<'q>>,
    ) -> sqlx::encode::IsNull {
        args.push(sqlx::sqlite::SqliteArgumentValue::Text(Cow::Owned(
            self.to_string(),
        )));

        sqlx::encode::IsNull::No
    }
}

impl<'r> sqlx::Decode<'r, sqlx::Sqlite> for Type {
    fn decode(value: sqlx::sqlite::SqliteValueRef<'r>) -> Result<Self, sqlx::error::BoxDynError> {
        let value = <&str as sqlx::Decode<sqlx::Sqlite>>::decode(value)?;

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
            Type::Turso,
        ];

        for input in inputs {
            let actual = Type::from_str(&input.to_string()).unwrap();
            assert_eq!(input, actual, ":{} should map back to itself", input);
        }
    }
}
