use std::time::Duration;

use async_trait::async_trait;
use bollard::{container::StopContainerOptions, service::ContainerInspectResponse};
use serde::{Deserialize, Serialize};
use tokio::time::sleep;
use tracing::{debug, instrument};

use crate::{
    project::{docker::DockerContext, service::state::starting::ServiceStarting},
    safe_unwrap,
};

use super::errored::ServiceErrored;
use super::machine::State;

const MAX_RESTARTS: usize = 5;

/// Special state for when `ProjectStarting` fails to retry it
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ServiceRestarting {
    pub container: ContainerInspectResponse,
    pub restart_count: usize,
}

#[async_trait]
impl<Ctx> State<Ctx> for ServiceRestarting
where
    Ctx: DockerContext,
{
    type Next = ServiceStarting;
    type Error = ServiceErrored;

    #[instrument(skip_all)]
    async fn next(self, ctx: &Ctx) -> Result<Self::Next, Self::Error> {
        let Self {
            container,
            restart_count,
        } = self;

        let container_id = safe_unwrap!(container.id);

        // Stop it just to be safe
        ctx.docker()
            .stop_container(container_id, Some(StopContainerOptions { t: 1 }))
            .await
            .unwrap_or(());

        debug!("project restarted {} times", restart_count);

        if restart_count < MAX_RESTARTS {
            sleep(Duration::from_secs(5)).await;
            Ok(ServiceStarting {
                container,
                restart_count: restart_count + 1,
            })
        } else {
            Err(ServiceErrored::internal("too many restarts"))
        }
    }
}
