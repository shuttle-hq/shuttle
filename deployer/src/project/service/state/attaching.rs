use crate::{
    project::docker::{ContainerSettings, DockerContext},
    safe_unwrap,
};
use async_trait::async_trait;
use bollard::{errors::Error as DockerError, network::ConnectNetworkOptions};
use bollard::{network::DisconnectNetworkOptions, service::ContainerInspectResponse};
use serde::{Deserialize, Serialize};
use tracing::{error, info, instrument};

use super::{
    errored::ServiceErrored,
    machine::{Refresh, State},
    starting::ServiceStarting,
};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ServiceAttaching {
    pub container: ContainerInspectResponse,
    // Use default for backward compatibility. Can be removed when all projects in the DB have this property set
    #[serde(default)]
    pub recreate_count: usize,
}

#[async_trait]
impl<Ctx> State<Ctx> for ServiceAttaching
where
    Ctx: DockerContext,
{
    type Next = ServiceStarting;
    type Error = ServiceErrored;

    #[instrument(skip_all)]
    async fn next(self, ctx: &Ctx) -> Result<Self::Next, Self::Error> {
        let Self { container, .. } = self;

        let container_id = safe_unwrap!(container.id);
        let ContainerSettings { network_name, .. } = ctx.container_settings();

        // Disconnect the bridge network before trying to start up
        // For docker bug https://github.com/docker/cli/issues/1891
        //
        // Also disconnecting from all network because docker just losses track of their IDs sometimes when restarting
        for network in safe_unwrap!(container.network_settings.networks).keys() {
            ctx.docker().disconnect_network(network, DisconnectNetworkOptions {
            container: container_id,
            force: true,
        })
            .await
            .or_else(|err| {
                if matches!(err, DockerError::DockerResponseServerError { status_code, .. } if status_code == 500) {
                    info!("already disconnected from the {network} network");
                    Ok(())
                } else {
                    Err(err)
                }
            })?;
        }

        // Make sure the container is connected to the user network
        let network_config = ConnectNetworkOptions {
            container: container_id,
            endpoint_config: Default::default(),
        };
        ctx.docker()
            .connect_network(network_name, network_config)
            .await
            .or_else(|err| {
                if matches!(
                    err,
                    DockerError::DockerResponseServerError { status_code, .. } if status_code == 409
                ) {
                    info!("already connected to the shuttle network");
                    Ok(())
                } else {
                    error!(
                        error = &err as &dyn std::error::Error,
                        "failed to connect to shuttle network"
                    );
                    Err(ServiceErrored::no_network(
                        "failed to connect to shuttle network",
                    ))
                }
            })?;

        let container = container.refresh(ctx).await?;

        Ok(ServiceStarting {
            container,
            restart_count: 0,
        })
    }
}
