use std::{collections::VecDeque, net::IpAddr};

use async_trait::async_trait;
use bollard::{container::Stats, service::ContainerInspectResponse};
use serde::{Deserialize, Serialize};
use tracing::instrument;

use super::machine::State;
use crate::project::{docker::DockerContext, service::Service};

use super::errored::ServiceErrored;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ServiceReady {
    container: ContainerInspectResponse,
    service: Service,
    // Use default for backward compatibility. Can be removed when all projects in the DB have this property set
    #[serde(default)]
    stats: VecDeque<Stats>,
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
    pub fn target_ip(&self) -> &IpAddr {
        &self.service.target
    }

    pub async fn is_healthy(&mut self) -> bool {
        self.service.is_healthy().await
    }
}
