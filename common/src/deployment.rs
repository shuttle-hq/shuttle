use serde::{Deserialize, Serialize};
use strum::Display;
use utoipa::ToSchema;

#[derive(Clone, Debug, Deserialize, Display, Serialize, ToSchema)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
#[schema(as = shuttle_common::deployment::State)]
pub enum State {
    Queued,
    Building,
    Built,
    Loading,
    Running,
    Completed,
    Stopped,
    Crashed,
    Unknown,
}

/// This which environment is this deployment taking place
#[derive(Clone, Copy)]
pub enum Environment {
    Local,
    Production,
}
