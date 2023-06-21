use std::{collections::VecDeque, net::Ipv4Addr};

use crate::{
    project::{docker::DockerContext, service::Service},
    runtime_manager::RuntimeManager,
};
use async_trait::async_trait;
use bollard::{container::Stats, service::ContainerInspectResponse};
use serde::{Deserialize, Serialize};
use tracing::instrument;

use super::{
    f_running::ServiceRunning,
    m_errored::ServiceErrored,
    machine::{Refresh, State},
    StateVariant,
};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ServiceReady {
    pub container: ContainerInspectResponse,
    pub service: Service,
    // Use default for backward compatibility. Can be removed when all projects in the DB have this property set
    #[serde(default)]
    pub stats: VecDeque<Stats>,
}

impl StateVariant for ServiceReady {
    fn name() -> String {
        "Ready".to_string()
    }

    fn as_state_variant(&self) -> String {
        Self::name()
    }
}

#[async_trait]
impl<Ctx> State<Ctx> for ServiceReady
where
    Ctx: DockerContext,
{
    type Next = ServiceRunning;
    type Error = ServiceErrored;

    #[instrument(skip_all)]
    async fn next(mut self, ctx: &Ctx) -> Result<Self::Next, Self::Error> {
        let target_ip = self.target_ip();
        let Self {
            container,
            service,
            stats,
        } = self;

        let cs = ctx.container_settings().ok_or(ServiceErrored::internal(
            "failed to get the container settings in the ready state",
        ))?;
        let mut runnable_deployment = cs.runnable_deployment.clone();
        runnable_deployment.target_ip = Some(target_ip);
        cs.runtime_start_channel
            .send(runnable_deployment)
            .await
            .map_err(|err| {
                ServiceErrored::internal(format!("failed to start the runtime: {}", err))
            })?;
        Ok(ServiceRunning {
            container: container.refresh(ctx).await.map_err(|err| {
                ServiceErrored::internal(format!(
                    "failed to inspect the container when transitioning to the running state: {}",
                    err
                ))
            })?,
            service,
            stats,
        })
    }
}

impl ServiceReady {
    pub fn target_ip(&self) -> Ipv4Addr {
        self.service.target
    }

    pub async fn is_healthy(
        &mut self,
        runtime_manager: RuntimeManager,
    ) -> Result<bool, super::super::error::Error> {
        self.service.is_healthy(runtime_manager).await
    }
}
