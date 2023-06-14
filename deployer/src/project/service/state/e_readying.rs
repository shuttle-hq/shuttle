use serde::{Deserialize, Serialize};

use super::{d_started::ServiceStarted, f_ready::ServiceReady, k_stopping::ServiceStopping};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ServiceReadying {
    Ready(ServiceReady),
    Started(ServiceStarted),
    Idle(ServiceStopping),
}
