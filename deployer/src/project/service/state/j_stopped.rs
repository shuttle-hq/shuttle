use async_trait::async_trait;
use bollard::service::ContainerInspectResponse;
use serde::{Deserialize, Serialize};
use tracing::instrument;

use super::{machine::State, StateVariant};
use crate::project::docker::DockerContext;

use super::m_errored::ServiceErrored;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ServiceStopped {
    pub container: ContainerInspectResponse,
}

impl StateVariant for ServiceStopped {
    fn name() -> String {
        "Stopped".to_string()
    }
}

#[async_trait]
impl<Ctx> State<Ctx> for ServiceStopped
where
    Ctx: DockerContext,
{
    type Next = ServiceStopped;
    type Error = ServiceErrored;

    #[instrument(skip_all)]
    async fn next(self, _ctx: &Ctx) -> Result<Self::Next, Self::Error> {
        Ok(self)
    }
}
