use std::{fmt::Display, str::FromStr};

use serde::{Deserialize, Serialize};
use strum::{Display, EnumString};
#[cfg(feature = "openapi")]
use utoipa::ToSchema;

use crate::split_first_component;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[cfg_attr(feature = "openapi", schema(as = shuttle_common::database::Type))]
pub enum Type {
    AwsRds(AwsRdsEngine),
    Shared(SharedEngine),
}

#[derive(Clone, Copy, Debug, Deserialize, Display, EnumString, Serialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub enum AwsRdsEngine {
    Postgres,
    MySql,
    MariaDB,
}

#[derive(Clone, Copy, Debug, Deserialize, Display, EnumString, Serialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub enum SharedEngine {
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
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match split_first_component(s) {
            ("aws_rds", Some(rds_type)) => {
                Ok(Type::AwsRds(rds_type.parse().map_err(|_| ParseError)?))
            }
            ("shared", Some(shared_type)) => {
                Ok(Type::Shared(shared_type.parse().map_err(|_| ParseError)?))
            }
            _ => Err(ParseError),
        }
    }
}

pub struct ParseError;
