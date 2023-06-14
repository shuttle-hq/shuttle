use async_trait::async_trait;
use bollard::{
    container::{RemoveContainerOptions, StopContainerOptions},
    service::ContainerInspectResponse,
};
use serde::{Deserialize, Serialize};
use tracing::instrument;

use super::machine::State;
use crate::{project::docker::DockerContext, safe_unwrap};

use super::{m_destroyed::ServiceDestroyed, m_errored::ServiceErrored};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ServiceDestroying {
    pub container: ContainerInspectResponse,
}

#[async_trait]
impl<Ctx> State<Ctx> for ServiceDestroying
where
    Ctx: DockerContext,
{
    type Next = ServiceDestroyed;
    type Error = ServiceErrored;

    #[instrument(skip_all)]
    async fn next(self, ctx: &Ctx) -> Result<Self::Next, Self::Error> {
        let Self { container } = self;
        let container_id = safe_unwrap!(container.id);
        ctx.docker()
            .stop_container(container_id, Some(StopContainerOptions { t: 1 }))
            .await
            .unwrap_or(());
        ctx.docker()
            .remove_container(
                container_id,
                Some(RemoveContainerOptions {
                    force: true,
                    ..Default::default()
                }),
            )
            .await
            .unwrap_or(());
        Ok(Self::Next {
            destroyed: Some(container),
        })
    }
}
