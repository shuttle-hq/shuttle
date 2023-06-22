use serde::{Deserialize, Serialize};

use super::{d_started::ServiceStarted, f_ready::ServiceReady, StateVariant};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ServiceReadying {
    Ready(ServiceReady),
    Started(ServiceStarted),
}

impl StateVariant for ServiceReadying {
    fn name() -> String {
        "Readying".to_string()
    }
}
