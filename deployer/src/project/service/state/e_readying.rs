use serde::{Deserialize, Serialize};

use super::{
    d_started::ServiceStarted, f_ready::ServiceReady, k_stopping::ServiceStopping, StateVariant,
};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ServiceReadying {
    Ready(ServiceReady),
    Started(ServiceStarted),
    Idle(ServiceStopping),
}

impl StateVariant for ServiceReadying {
    fn name() -> String {
        "Readying".to_string()
    }

    fn as_state_variant(&self) -> String {
        Self::name()
    }
}
