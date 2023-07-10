use serde::{Deserialize, Serialize};

use sqlx::sqlite::SqliteSynchronous;

#[derive(Debug, Deserialize, Serialize)]
pub enum ShuttleSqliteSynchronous {
    Off,
    Normal,
    Full,
    Extra,
}

impl Default for ShuttleSqliteSynchronous {
    fn default() -> Self {
        ShuttleSqliteSynchronous::Full
    }
}

impl From<&ShuttleSqliteSynchronous> for SqliteSynchronous {
    fn from(value: &ShuttleSqliteSynchronous) -> Self {
        match value {
            ShuttleSqliteSynchronous::Off => SqliteSynchronous::Off,
            ShuttleSqliteSynchronous::Normal => SqliteSynchronous::Normal,
            ShuttleSqliteSynchronous::Full => SqliteSynchronous::Full,
            ShuttleSqliteSynchronous::Extra => SqliteSynchronous::Extra,
        }
    }
}
