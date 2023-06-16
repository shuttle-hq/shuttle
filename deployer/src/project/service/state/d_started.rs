use std::collections::VecDeque;

use async_trait::async_trait;
use bollard::{
    container::{Stats, StatsOptions},
    service::ContainerInspectResponse,
};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use shuttle_proto::runtime::Ping;
use tracing::{debug, instrument};
use ulid::Ulid;

use crate::{
    project::{
        docker::{ContainerInspectResponseExt, DockerContext},
        service::state::k_stopping::ServiceStopping,
        service::Service,
    },
    safe_unwrap,
};

use super::machine::{Refresh, State};
use super::{e_readying::ServiceReadying, f_ready::ServiceReady, m_errored::ServiceErrored};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ServiceStarted {
    pub container: ContainerInspectResponse,
    service: Option<Service>,
    // Use default for backward compatibility. Can be removed when all projects in the DB have this property set
    #[serde(default)]
    pub stats: VecDeque<Stats>,
}

impl ServiceStarted {
    pub fn new(container: ContainerInspectResponse, stats: VecDeque<Stats>) -> Self {
        Self {
            container,
            service: None,
            stats,
        }
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
        let Self {
            container,
            service,
            mut stats,
        } = self;
        let container = container.refresh(ctx).await?;
        let service = match service {
            Some(service) => service,
            None => Service::from_container(container.clone())?,
        };

        let service_id = Ulid::from_string(service.id.as_str())
            .map_err(|_| ServiceErrored::internal("failed to get the service id"))?;

        let mut runtime_client = ctx
            .runtime_manager()
            .runtime_client(service_id, service.target)
            .await
            .expect("to create a runtime client");
        debug!("oare aici sta?");
        if runtime_client.health_check(Ping {}).await.is_ok() {
            debug!("the service runtime responded to health check");
            let idle_minutes = container.idle_minutes();

            // Idle minutes of `0` means it is disabled and the project will always stay up
            if idle_minutes < 1 {
                debug!("idle minutes < 1");
                Ok(Self::Next::Ready(ServiceReady {
                    container,
                    service,
                    stats,
                }))
            } else {
                debug!("new stats");
                let new_stat = ctx
                    .docker()
                    .stats(
                        safe_unwrap!(container.id),
                        Some(StatsOptions {
                            one_shot: true,
                            stream: false,
                        }),
                    )
                    .next()
                    .await
                    .unwrap()?;

                stats.push_back(new_stat.clone());

                let mut last = None;

                while stats.len() > (idle_minutes as usize) {
                    last = stats.pop_front();
                }

                if let Some(last) = last {
                    let cpu_per_minute = (new_stat.cpu_stats.cpu_usage.total_usage
                        - last.cpu_stats.cpu_usage.total_usage)
                        / idle_minutes;

                    debug!("{} has {} CPU usage per minute", service.id, cpu_per_minute);

                    // From analysis we know the following kind of CPU usage for different kinds of idle projects
                    // Web framework uses 6_200_000 CPU per minute
                    // Serenity uses 20_000_000 CPU per minute
                    //
                    // We want to make sure we are able to stop these kinds of projects
                    //
                    // Now, the following kind of CPU usage has been observed for different kinds of projects having
                    // 2 web requests / processing 2 discord messages per minute
                    // Web framework uses 100_000_000 CPU per minute
                    // Serenity uses 30_000_000 CPU per minute
                    //
                    // And projects at these levels we will want to keep active. However, the 30_000_000
                    // for an "active" discord will be to close to the 20_000_000 of an idle framework. And
                    // discord will have more traffic in anyway. So using the 100_000_000 threshold of an
                    // active framework for now
                    if cpu_per_minute < 100_000_000 {
                        Ok(Self::Next::Idle(ServiceStopping { container }))
                    } else {
                        Ok(Self::Next::Ready(ServiceReady {
                            container,
                            service,
                            stats,
                        }))
                    }
                } else {
                    debug!("service ready down");
                    Ok(Self::Next::Ready(ServiceReady {
                        container,
                        service,
                        stats,
                    }))
                }
            }
        } else {
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
                stats,
            }))
        }
    }
}
