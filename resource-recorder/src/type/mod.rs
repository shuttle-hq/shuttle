use std::{fmt::Display, str::FromStr};

pub mod database;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Type {
    Database(database::Type),
    Secrets,
    StaticFolder,
    Persist,
}

impl Display for Type {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Type::Database(db_type) => write!(f, "database::{db_type}"),
            Type::Secrets => write!(f, "secrets"),
            Type::StaticFolder => write!(f, "static_folder"),
            Type::Persist => write!(f, "persist"),
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
                _ => Err(format!("'{s}' is an unknown resource type")),
            }
        }
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
        ];

        for input in inputs {
            let actual = Type::from_str(&input.to_string()).unwrap();
            assert_eq!(input, actual, ":{} should map back to itself", input);
        }
    }
}
