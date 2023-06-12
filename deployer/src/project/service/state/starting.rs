use std::collections::VecDeque;

use async_trait::async_trait;
use bollard::{errors::Error as DockerError, service::ContainerInspectResponse};
use serde::{Deserialize, Serialize};
use tracing::instrument;

use crate::{
    project::{
        docker::DockerContext,
        machine::{Refresh, State},
    },
    safe_unwrap,
};

use super::{errored::ServiceErrored, started::ServiceStarted};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ServiceStarting {
    container: ContainerInspectResponse,
    // Use default for backward compatibility. Can be removed when all projects in the DB have this property set
    #[serde(default)]
    restart_count: usize,
}

#[async_trait]
impl<Ctx> State<Ctx> for ServiceStarting
where
    Ctx: DockerContext,
{
    type Next = ServiceStarted;
    type Error = ServiceErrored;

    #[instrument(skip_all)]
    async fn next(self, ctx: &Ctx) -> Result<Self::Next, Self::Error> {
        let Self { container, .. } = self;
        let container_id = safe_unwrap!(container.id);

        ctx.docker()
            .start_container::<String>(container_id, None)
            .await
            .or_else(|err| {
                if matches!(err, DockerError::DockerResponseServerError { status_code, .. } if status_code == 304) {
                    // Already started
                    Ok(())
                } else {
                    Err(err)
                }
            })?;

        let container = container.refresh(ctx).await?;

        Ok(Self::Next::new(container, VecDeque::new()))
    }
}
