use serde::{Deserialize, Serialize};
use strum::Display;

#[derive(Clone, Debug, Deserialize, Display, Serialize)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
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
