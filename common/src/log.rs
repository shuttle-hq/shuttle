use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::deployment::State;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct StreamLog {
    pub id: Uuid,
    pub state: State,
    pub message: Option<String>,
    pub timestamp: DateTime<Utc>,
}
