use strum::{Display, EnumString};

/// States a deployment can be in
#[derive(sqlx::Type, Debug, Display, Clone, Copy, EnumString, PartialEq, Eq)]
pub enum State {
    /// Deployment is built, but has not been started yet
    Built,

    /// Deployment is being loaded and resources are provisioned
    Loading,

    /// Deployment is running - ie. its thread is active
    Running,

    /// Deployment was running, but stopped running all by itself. This is expected for things like background workers
    Completed,

    /// Deployment was running, but has been stopped by the user.
    Stopped,

    /// Something in the deployment process failed
    Crashed,

    /// We never expect this state and entering this state should be considered a bug
    Unknown,
}

impl Default for State {
    fn default() -> Self {
        Self::Unknown
    }
}
