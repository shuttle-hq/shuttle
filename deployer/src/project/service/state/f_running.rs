use std::{collections::VecDeque, net::Ipv4Addr};

use crate::project::{docker::DockerContext, service::Service};
use async_trait::async_trait;
use bollard::{container::Stats, service::ContainerInspectResponse};
use serde::{Deserialize, Serialize};
use tracing::instrument;

use super::{m_errored::ServiceErrored, machine::State, StateVariant};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ServiceRunning {
    pub container: ContainerInspectResponse,
    pub service: Service,
    // Use default for backward compatibility. Can be removed when all projects in the DB have this property set
    #[serde(default)]
    pub stats: VecDeque<Stats>,
}

impl StateVariant for ServiceRunning {
    fn name() -> String {
        "Running".to_string()
    }

    fn as_state_variant(&self) -> String {
        Self::name()
    }
}

#[async_trait]
impl<Ctx> State<Ctx> for ServiceRunning
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

impl ServiceRunning {
    pub fn target_ip(&self) -> Ipv4Addr {
        self.service.target
    }
}
