use serde::{Deserialize, Serialize};

use super::{ready::ServiceReady, started::ServiceStarted, stopping::ServiceStopping};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ServiceReadying {
    Ready(ServiceReady),
    Started(ServiceStarted),
    Idle(ServiceStopping),
}
