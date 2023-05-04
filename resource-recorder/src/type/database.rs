use std::{fmt::Display, str::FromStr};

use strum::{Display, EnumString};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Type {
    AwsRds(AwsRdsType),
    Shared(SharedType),
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

impl Display for Type {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Type::AwsRds(rds_type) => write!(f, "aws_rds::{rds_type}"),
            Type::Shared(shared_type) => write!(f, "shared::{shared_type}"),
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
