use std::{collections::VecDeque, net::Ipv4Addr, sync::Arc};

use async_trait::async_trait;
use bollard::{container::Stats, service::ContainerInspectResponse};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use tracing::instrument;

use super::machine::State;
use crate::{
    project::{docker::DockerContext, service::Service},
    runtime_manager::RuntimeManager,
};

use super::m_errored::ServiceErrored;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ServiceReady {
    pub container: ContainerInspectResponse,
    pub service: Service,
    // Use default for backward compatibility. Can be removed when all projects in the DB have this property set
    #[serde(default)]
    pub stats: VecDeque<Stats>,
}

#[async_trait]
impl<Ctx> State<Ctx> for ServiceReady
where
    Ctx: DockerContext,
{
    type Next = Self;
    type Error = ServiceErrored;

    #[instrument(skip_all)]
    async fn next(mut self, _ctx: &Ctx) -> Result<Self::Next, Self::Error> {
        Ok(self)
    }
}

impl ServiceReady {
    pub fn target_ip(&self) -> Ipv4Addr {
        self.service.target
    }

    pub async fn is_healthy(
        &mut self,
        runtime_manager: Arc<Mutex<RuntimeManager>>,
    ) -> Result<bool, super::super::error::Error> {
        self.service.is_healthy(runtime_manager).await
    }
}
