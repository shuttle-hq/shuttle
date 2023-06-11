use serde::{Deserialize, Serialize};

use super::state::creating::ServiceCreating;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Service {
    Creating(ServiceCreating),
    // Attaching(ProjectAttaching),
    // Recreating(ProjectRecreating),
    // Starting(ProjectStarting),
    // Restarting(ProjectRestarting),
    // Started(ProjectStarted),
    // Ready(ProjectReady),
    // Rebooting(ProjectRebooting),
    // Stopping(ProjectStopping),
    // Stopped(ProjectStopped),
    // Destroying(ProjectDestroying),
    // Destroyed(ProjectDestroyed),
    // Errored(ProjectError),
}
