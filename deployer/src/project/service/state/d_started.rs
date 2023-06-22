use async_trait::async_trait;
use bollard::service::ContainerInspectResponse;
use serde::{Deserialize, Serialize};
use shuttle_proto::runtime::Ping;
use tracing::{debug, instrument};
use ulid::Ulid;

use crate::{
    project::{docker::DockerContext, service::Service},
    safe_unwrap,
};

use super::{e_readying::ServiceReadying, f_ready::ServiceReady, m_errored::ServiceErrored};
use super::{
    machine::{Refresh, State},
    StateVariant,
};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ServiceStarted {
    pub container: ContainerInspectResponse,
    pub service: Option<Service>,
}

impl ServiceStarted {
    pub fn new(container: ContainerInspectResponse) -> Self {
        Self {
            container,
            service: None,
        }
    }
}

impl StateVariant for ServiceStarted {
    fn name() -> String {
        "Started".to_string()
    }
}

#[async_trait]
impl<Ctx> State<Ctx> for ServiceStarted
where
    Ctx: DockerContext,
{
    type Next = ServiceReadying;
    type Error = ServiceErrored;

    #[instrument(skip_all)]
    async fn next(self, ctx: &Ctx) -> Result<Self::Next, Self::Error> {
        let Self { container, service } = self;
        let container = container.refresh(ctx).await?;
        let service = match service {
            Some(service) => service,
            None => Service::from_container(container.clone())?,
        };

        let service_id = Ulid::from_string(service.id.as_str())
            .map_err(|_| ServiceErrored::internal("failed to get the service id"))?;

        if let Ok(mut runtime_client) = ctx
            .runtime_manager()
            .runtime_client(service_id, service.target)
            .await
        {
            // If healt-check works, move it to ready.
            if runtime_client.health_check(Ping {}).await.is_ok() {
                return Ok(Self::Next::Ready(ServiceReady { container, service }));
            }
        }

        // Otherwise, try checking how much time has passed since we started the container.
        debug!("the service runtime didn't respond to health check");
        let started_at =
            chrono::DateTime::parse_from_rfc3339(safe_unwrap!(container.state.started_at))
                .map_err(|_err| {
                    ServiceErrored::internal("invalid `started_at` response from Docker daemon")
                })?;
        let now = chrono::offset::Utc::now();
        if started_at + chrono::Duration::seconds(120) < now {
            return Err(ServiceErrored::internal(
                "project did not become healthy in time",
            ));
        }

        Ok(Self::Next::Started(ServiceStarted {
            container,
            service: Some(service),
        }))
    }
}
