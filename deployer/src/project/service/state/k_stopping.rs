use async_trait::async_trait;
use bollard::{container::KillContainerOptions, service::ContainerInspectResponse};
use serde::{Deserialize, Serialize};
use tracing::instrument;

use super::machine::{Refresh, State};
use super::{j_stopped::ServiceStopped, m_errored::ServiceErrored};
use crate::{project::docker::DockerContext, safe_unwrap};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ServiceStopping {
    pub container: ContainerInspectResponse,
}

#[async_trait]
impl<Ctx> State<Ctx> for ServiceStopping
where
    Ctx: DockerContext,
{
    type Next = ServiceStopped;

    type Error = ServiceErrored;

    #[instrument(skip_all)]
    async fn next(self, ctx: &Ctx) -> Result<Self::Next, Self::Error> {
        let Self { container } = self;

        // Stopping a docker containers sends a SIGTERM which will stop the tokio runtime that deployer starts up.
        // Killing this runtime causes the deployment to enter the `completed` state and it therefore does not
        // start up again when starting up the project's container. Luckily the kill command allows us to change the
        // signal to prevent this from happening.
        //
        // In some future state when all deployers hadle `SIGTERM` correctly, this can be changed to docker stop
        // safely.
        ctx.docker()
            .kill_container(
                safe_unwrap!(container.id),
                Some(KillContainerOptions { signal: "SIGKILL" }),
            )
            .await?;
        Ok(Self::Next {
            container: container.refresh(ctx).await?,
        })
    }
}
