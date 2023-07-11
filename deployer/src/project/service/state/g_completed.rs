use async_trait::async_trait;
use bollard::service::ContainerInspectResponse;
use serde::{Deserialize, Serialize};
use tracing::instrument;

use super::{machine::State, StateVariant};
use crate::project::docker::DockerContext;

use super::m_errored::ServiceErrored;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ServiceCompleted {
    pub container: ContainerInspectResponse,
}

impl ServiceCompleted {
    pub fn from_container(container: ContainerInspectResponse) -> Result<Self, ServiceErrored> {
        Ok(Self { container })
    }
}

impl StateVariant for ServiceCompleted {
    fn name() -> String {
        "Completed".to_string()
    }
}

#[async_trait]
impl<Ctx> State<Ctx> for ServiceCompleted
where
    Ctx: DockerContext,
{
    type Next = ServiceCompleted;
    type Error = ServiceErrored;

    #[instrument(skip_all)]
    async fn next(self, _ctx: &Ctx) -> Result<Self::Next, Self::Error> {
        Ok(self)
    }
}
