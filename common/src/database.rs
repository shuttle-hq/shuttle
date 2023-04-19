use std::fmt::Display;

use serde::{Deserialize, Serialize};
use strum::Display;
use utoipa::ToSchema;

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq, ToSchema)]
#[serde(rename_all = "lowercase")]
#[schema(as = shuttle_common::database::Type)]
pub enum Type {
    AwsRds(AwsRdsEngine),
    Shared(SharedEngine),
}

#[derive(Clone, Debug, Deserialize, Display, Serialize, Eq, PartialEq, ToSchema)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
pub enum AwsRdsEngine {
    Postgres,
    MySql,
    MariaDB,
}

#[derive(Clone, Debug, Deserialize, Display, Serialize, Eq, PartialEq, ToSchema)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
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
