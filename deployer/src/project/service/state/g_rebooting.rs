use std::collections::HashMap;

use async_trait::async_trait;
use bollard::{
    container::StopContainerOptions, service::ContainerInspectResponse, system::EventsOptions,
};
use futures::TryStreamExt;
use serde::{Deserialize, Serialize};
use tracing::{debug, instrument};

use super::machine::{Refresh, State};
use crate::{
    project::{docker::DockerContext, service::state::m_errored::ServiceErrored},
    safe_unwrap,
};

use super::c_starting::ServiceStarting;

const MAX_REBOOTS: usize = 3;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ServiceRebooting {
    pub container: ContainerInspectResponse,
}

#[async_trait]
impl<Ctx> State<Ctx> for ServiceRebooting
where
    Ctx: DockerContext,
{
    type Next = ServiceStarting;

    type Error = ServiceErrored;

    #[instrument(skip_all)]
    async fn next(self, ctx: &Ctx) -> Result<Self::Next, Self::Error> {
        let Self { mut container } = self;
        ctx.docker()
            .stop_container(
                safe_unwrap!(container.id),
                Some(StopContainerOptions { t: 30 }),
            )
            .await?;

        container = container.refresh(ctx).await?;
        let since = (chrono::Utc::now() - chrono::Duration::minutes(15))
            .timestamp()
            .to_string();
        let until = chrono::Utc::now().timestamp().to_string();

        // Filter and collect `start` events for this project in the last 15 minutes
        let start_events = ctx
            .docker()
            .events(Some(EventsOptions::<&str> {
                since: Some(since),
                until: Some(until),
                filters: HashMap::from([
                    ("container", vec![safe_unwrap!(container.id).as_str()]),
                    ("event", vec!["start"]),
                ]),
            }))
            .try_collect::<Vec<_>>()
            .await?;

        let start_event_count = start_events.len();
        debug!(
            "project started {} times in the last 15 minutes",
            start_event_count
        );

        // If stopped, and has not restarted too much, try to restart
        if start_event_count < MAX_REBOOTS {
            Ok(ServiceStarting {
                container,
                restart_count: 0,
            })
        } else {
            Err(ServiceErrored::internal(
                "too many restarts in the last 15 minutes",
            ))
        }
    }
}
