use async_trait::async_trait;
use bollard::service::ContainerInspectResponse;
use serde::{Deserialize, Serialize};
use tracing::instrument;

use super::{machine::State, StateVariant};
use crate::project::docker::DockerContext;

use super::m_errored::ServiceErrored;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ServiceDestroyed {
    pub destroyed: Option<ContainerInspectResponse>,
}

impl StateVariant for ServiceDestroyed {
    fn name() -> String {
        "Destroyed".to_string()
    }
}

#[async_trait]
impl<Ctx> State<Ctx> for ServiceDestroyed
where
    Ctx: DockerContext,
{
    type Next = ServiceDestroyed;
    type Error = ServiceErrored;

    #[instrument(skip_all)]
    async fn next(self, _ctx: &Ctx) -> Result<Self::Next, Self::Error> {
        Ok(self)
    }
}
