use std::time::Duration;

use async_trait::async_trait;
use bollard::{
    container::{RemoveContainerOptions, StopContainerOptions},
    service::ContainerInspectResponse,
};
use serde::{Deserialize, Serialize};
use tokio::time::sleep;
use tracing::instrument;

use super::machine::State;
use crate::{project::docker::DockerContext, safe_unwrap};

use super::{a_creating::ServiceCreating, m_errored::ServiceErrored};

const MAX_RECREATES: usize = 5;

// Special state to try and recreate a container if it failed to be created
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ServiceRecreating {
    pub container: ContainerInspectResponse,
    pub recreate_count: usize,
}

#[async_trait]
impl<Ctx> State<Ctx> for ServiceRecreating
where
    Ctx: DockerContext,
{
    type Next = ServiceCreating;
    type Error = ServiceErrored;

    #[instrument(skip_all)]
    async fn next(self, ctx: &Ctx) -> Result<Self::Next, Self::Error> {
        let Self {
            container,
            recreate_count,
        } = self;
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

        if recreate_count < MAX_RECREATES {
            sleep(Duration::from_secs(5)).await;
            Ok(ServiceCreating::from_container(
                container,
                recreate_count + 1,
            )?)
        } else {
            Err(ServiceErrored::internal("too many recreates"))
        }
    }
}
