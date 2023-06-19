use serde::{Deserialize, Serialize};

use super::{
    d_started::ServiceStarted, f_running::ServiceRunning, k_stopping::ServiceStopping, StateVariant,
};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ServiceReadying {
    Ready(ServiceRunning),
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
