use std::{
    collections::VecDeque,
    convert::Infallible,
    fmt::Display,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::Arc,
};

use async_trait::async_trait;
use bollard::errors::Error as DockerError;
use bollard::service::{ContainerInspectResponse, ContainerStateStatusEnum};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use tracing::{debug, error, instrument};
use ulid::Ulid;

use crate::runtime_manager::RuntimeManager;

use self::{
    error::Error,
    state::{
        a_creating::ServiceCreating,
        b_attaching::ServiceAttaching,
        c_starting::ServiceStarting,
        d_started::ServiceStarted,
        e_readying::ServiceReadying,
        f_ready::ServiceReady,
        g_rebooting::ServiceRebooting,
        h_recreating::ServiceRecreating,
        i_restarting::ServiceRestarting,
        j_stopped::ServiceStopped,
        k_stopping::ServiceStopping,
        l_destroying::ServiceDestroying,
        m_destroyed::ServiceDestroyed,
        m_errored::{ServiceErrored, ServiceErroredKind},
    },
};

use super::docker::{ContainerInspectResponseExt, DockerContext};
use state::machine::{EndState, IntoTryState, Refresh, State, TryState};

pub mod error;
pub mod state;

// shuttle-runtime default port
pub const RUNTIME_API_PORT: u16 = 8001;

#[macro_export]
macro_rules! safe_unwrap {
    {$fst:ident$(.$attr:ident$(($ex:expr))?)+} => {
        $fst$(
            .$attr$(($ex))?
                .as_ref()
                .ok_or_else(|| ServiceErrored::internal(
                    concat!("container state object is malformed at attribute: ", stringify!($attr))
                ))?
        )+
    }
}

#[macro_export]
macro_rules! deserialize_json {
    {$ty:ty: $($json:tt)+} => {{
        let __ty_json = serde_json::json!($($json)+);
        serde_json::from_value::<$ty>(__ty_json).unwrap()
    }};
    {$($json:tt)+} => {{
        let __ty_json = serde_json::json!($($json)+);
        serde_json::from_value(__ty_json).unwrap()
    }}
}

macro_rules! impl_from_variant {
    {$e:ty: $($s:ty => $v:ident $(,)?)+} => {
        $(
            impl From<$s> for $e {
                fn from(s: $s) -> $e {
                    <$e>::$v(s)
                }
            }
        )+
    };
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ServiceState {
    Creating(ServiceCreating),
    Attaching(ServiceAttaching),
    Recreating(ServiceRecreating),
    Starting(ServiceStarting),
    Restarting(ServiceRestarting),
    Started(ServiceStarted),
    Ready(ServiceReady),
    Rebooting(ServiceRebooting),
    Stopping(ServiceStopping),
    Stopped(ServiceStopped),
    Destroying(ServiceDestroying),
    Destroyed(ServiceDestroyed),
    Errored(ServiceErrored),
}

impl_from_variant!(ServiceState:
    ServiceCreating => Creating,
    ServiceAttaching => Attaching,
    ServiceRecreating => Recreating,
    ServiceStarting => Starting,
    ServiceRestarting => Restarting,
    ServiceStarted => Started,
    ServiceReady => Ready,
    ServiceStopping => Stopping,
    ServiceStopped => Stopped,
    ServiceRebooting => Rebooting,
    ServiceDestroying => Destroying,
    ServiceDestroyed => Destroyed,
    ServiceErrored => Errored);

impl Display for ServiceState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self {
            ServiceState::Creating(_) => write!(f, "Creating"),
            ServiceState::Attaching(_) => write!(f, "Attaching"),
            ServiceState::Recreating(_) => write!(f, "Recreating"),
            ServiceState::Starting(_) => write!(f, "Starting"),
            ServiceState::Restarting(_) => write!(f, "Restarting"),
            ServiceState::Started(_) => write!(f, "Started"),
            ServiceState::Ready(_) => write!(f, "Ready"),
            ServiceState::Rebooting(_) => write!(f, "Rebooting"),
            ServiceState::Stopping(_) => write!(f, "Stopping"),
            ServiceState::Stopped(_) => write!(f, "Stopped"),
            ServiceState::Destroying(_) => write!(f, "Destroying"),
            ServiceState::Destroyed(_) => write!(f, "Destroyed"),
            ServiceState::Errored(_) => write!(f, "Errored"),
        }
    }
}

impl ServiceState {
    pub fn stop(self) -> Result<Self, Error> {
        if let Some(container) = self.container() {
            Ok(Self::Stopping(ServiceStopping { container }))
        } else {
            Err(Error::InvalidOperation(format!(
                "cannot stop a project in the `{}` state",
                self.state()
            )))
        }
    }

    pub fn reboot(self) -> Result<Self, Error> {
        if let Some(container) = self.container() {
            Ok(Self::Rebooting(ServiceRebooting { container }))
        } else {
            Err(Error::InvalidOperation(format!(
                "cannot reboot a project in the `{}` state",
                self.state()
            )))
        }
    }

    pub fn destroy(self) -> Result<Self, Error> {
        if let Some(container) = self.container() {
            Ok(Self::Destroying(ServiceDestroying { container }))
        } else {
            Ok(Self::Destroyed(ServiceDestroyed { destroyed: None }))
        }
    }

    pub fn start(self) -> Result<Self, Error> {
        if let Some(container) = self.container() {
            Ok(Self::Starting(ServiceStarting {
                container,
                restart_count: 0,
            }))
        } else {
            Err(Error::InvalidOperation(format!(
                "cannot start a project in the `{}` state",
                self.state()
            )))
        }
    }

    pub fn is_ready(&self) -> bool {
        matches!(self, Self::Ready(_))
    }

    pub fn is_destroyed(&self) -> bool {
        matches!(self, Self::Destroyed(_))
    }

    pub fn is_stopped(&self) -> bool {
        matches!(self, Self::Stopped(_))
    }

    pub fn target_ip(&self) -> Result<Option<Ipv4Addr>, Error> {
        match self.clone() {
            Self::Ready(project_ready) => Ok(Some(project_ready.target_ip())),
            _ => Ok(None), // not ready
        }
    }

    pub fn target_addr(&self) -> Result<Option<SocketAddr>, Error> {
        Ok(self
            .target_ip()?
            .map(|target_ip| SocketAddr::new(IpAddr::V4(target_ip), RUNTIME_API_PORT)))
    }

    pub fn state(&self) -> String {
        match self {
            Self::Started(_) => "started".to_string(),
            Self::Ready(_) => "ready".to_string(),
            Self::Stopped(_) => "stopped".to_string(),
            Self::Starting(ServiceStarting { restart_count, .. }) => {
                if *restart_count > 0 {
                    format!("starting (attempt {restart_count})")
                } else {
                    "starting".to_string()
                }
            }
            Self::Recreating(ServiceRecreating { recreate_count, .. }) => {
                format!("recreating (attempt {recreate_count})")
            }
            Self::Restarting(ServiceRestarting { restart_count, .. }) => {
                format!("restarting (attempt {restart_count})")
            }
            Self::Stopping(_) => "stopping".to_string(),
            Self::Rebooting(_) => "rebooting".to_string(),
            Self::Creating(ServiceCreating { recreate_count, .. }) => {
                if *recreate_count > 0 {
                    format!("creating (attempt {recreate_count})")
                } else {
                    "creating".to_string()
                }
            }
            Self::Attaching(ServiceAttaching { recreate_count, .. }) => {
                if *recreate_count > 0 {
                    format!("attaching (attempt {recreate_count})")
                } else {
                    "attaching".to_string()
                }
            }
            Self::Destroying(_) => "destroying".to_string(),
            Self::Destroyed(_) => "destroyed".to_string(),
            Self::Errored(_) => "error".to_string(),
        }
    }

    pub fn container(&self) -> Option<ContainerInspectResponse> {
        match self {
            Self::Starting(ServiceStarting { container, .. })
            | Self::Started(ServiceStarted { container, .. })
            | Self::Recreating(ServiceRecreating { container, .. })
            | Self::Restarting(ServiceRestarting { container, .. })
            | Self::Attaching(ServiceAttaching { container, .. })
            | Self::Ready(ServiceReady { container, .. })
            | Self::Stopping(ServiceStopping { container, .. })
            | Self::Stopped(ServiceStopped { container, .. })
            | Self::Rebooting(ServiceRebooting { container, .. })
            | Self::Destroying(ServiceDestroying { container }) => Some(container.clone()),
            Self::Errored(ServiceErrored { ctx: Some(ctx), .. }) => ctx.container(),
            Self::Errored(_) | Self::Creating(_) | Self::Destroyed(_) => None,
        }
    }

    pub fn container_id(&self) -> Option<String> {
        self.container().and_then(|container| container.id)
    }
}

#[async_trait]
impl<Ctx> State<Ctx> for ServiceState
where
    Ctx: DockerContext,
{
    type Next = Self;
    type Error = Infallible;

    #[instrument(skip_all, fields(state = %self.state()))]
    async fn next(self, ctx: &Ctx) -> Result<Self::Next, Self::Error> {
        let previous = self.clone();
        let previous_state = previous.state();

        let mut new = match self {
            Self::Creating(creating) => creating.next(ctx).await.into_try_state(),
            Self::Attaching(attaching) => match attaching.clone().next(ctx).await {
                Err(ServiceErrored {
                    kind: ServiceErroredKind::NoNetwork,
                    ..
                }) => {
                    // Recreate the container to try and connect to the network again
                    Ok(Self::Recreating(ServiceRecreating {
                        container: attaching.container,
                        recreate_count: attaching.recreate_count,
                    }))
                }
                attaching => attaching.into_try_state(),
            },
            Self::Recreating(recreating) => recreating.next(ctx).await.into_try_state(),
            Self::Starting(starting) => match starting.clone().next(ctx).await {
                Err(error) => {
                    error!(
                        error = &error as &dyn std::error::Error,
                        "project failed to start. Will restart it"
                    );

                    Ok(Self::Restarting(ServiceRestarting {
                        container: starting.container,
                        restart_count: starting.restart_count,
                    }))
                }
                starting => starting.into_try_state(),
            },
            Self::Restarting(restarting) => restarting.next(ctx).await.into_try_state(),
            Self::Started(started) => {
                debug!("blaaa");
                match started.next(ctx).await {
                    Ok(ServiceReadying::Ready(ready)) => {
                        debug!("in readying??");
                        Ok(ready.into())
                    }
                    Ok(ServiceReadying::Started(started)) => {
                        debug!("din nou in started");
                        Ok(started.into())
                    }
                    Ok(ServiceReadying::Idle(stopping)) => {
                        debug!("in stopping??");
                        Ok(stopping.into())
                    }
                    Err(err) => Ok(Self::Errored(err)),
                }
            }
            Self::Ready(ready) => ready.next(ctx).await.into_try_state(),
            Self::Stopped(stopped) => stopped.next(ctx).await.into_try_state(),
            Self::Stopping(stopping) => stopping.next(ctx).await.into_try_state(),
            Self::Rebooting(rebooting) => rebooting.next(ctx).await.into_try_state(),
            Self::Destroying(destroying) => destroying.next(ctx).await.into_try_state(),
            Self::Destroyed(destroyed) => destroyed.next(ctx).await.into_try_state(),
            Self::Errored(errored) => Ok(Self::Errored(errored)),
        };

        if let Ok(Self::Errored(errored)) = &mut new {
            errored.ctx = Some(Box::new(previous));
            error!(error = ?errored, "state for project errored");
        }

        let new_state = new.as_ref().unwrap().state();
        let container_id = new
            .as_ref()
            .unwrap()
            .container_id()
            .map(|id| format!("{id}: "))
            .unwrap_or_default();
        debug!("{container_id}{previous_state} -> {new_state}");

        new
    }
}

impl<Ctx> EndState<Ctx> for ServiceState
where
    Ctx: DockerContext,
{
    fn is_done(&self) -> bool {
        matches!(
            self,
            Self::Errored(_) | Self::Ready(_) | Self::Destroyed(_) | Self::Stopped(_)
        )
    }
}

impl TryState for ServiceState {
    type ErrorVariant = ServiceErrored;

    fn into_result(self) -> Result<Self, Self::ErrorVariant> {
        match self {
            Self::Errored(perr) => Err(perr),
            otherwise => Ok(otherwise),
        }
    }
}

#[async_trait]
impl<Ctx> Refresh<Ctx> for ServiceState
where
    Ctx: DockerContext,
{
    type Error = Error;

    /// TODO: we could be a bit more clever than this by using the
    /// health checks instead of matching against the raw container
    /// state which is probably prone to erroneously setting the
    /// project into the wrong state if the docker is transitioning
    /// the state of its resources under us
    async fn refresh(self, ctx: &Ctx) -> Result<Self, Self::Error> {
        let refreshed = match self {
            Self::Creating(creating) => Self::Creating(creating),
            Self::Attaching(attaching) => Self::Attaching(attaching),
            Self::Starting(ServiceStarting { container, restart_count }) => match container
                .clone()
                .refresh(ctx)
                .await
            {
                Ok(container) => match safe_unwrap!(container.state.status) {
                    ContainerStateStatusEnum::RUNNING => {
                        Self::Started(ServiceStarted::new(container, VecDeque::new()))
                    }
                    ContainerStateStatusEnum::CREATED => Self::Starting(ServiceStarting {
                        container,
                        restart_count,
                    }),
                    ContainerStateStatusEnum::EXITED => Self::Restarting(ServiceRestarting  { container, restart_count: 0 }),
                    _ => {
                        return Err(Error::Internal(
                            "container resource has drifted out of sync from the starting state: cannot recover".to_string(),
                        ))
                    }
                },
                Err(DockerError::DockerResponseServerError {
                    status_code: 404, ..
                }) => {
                    // container not found, let's try to recreate it
                    // with the same image
                    Self::Creating(ServiceCreating::from_container(container, 0)?)
                }
                Err(err) => return Err(Error::Docker(err)),
            },
            Self::Started(ServiceStarted { container, stats, .. })
            | Self::Ready(ServiceReady { container, stats, .. })
             => match container
                .clone()
                .refresh(ctx)
                .await
            {
                Ok(container) => match safe_unwrap!(container.state.status) {
                    ContainerStateStatusEnum::RUNNING => {
                        Self::Started(ServiceStarted::new(container, stats))
                    }
                    // Restart the container if it went down
                    ContainerStateStatusEnum::EXITED => Self::Restarting(ServiceRestarting  { container, restart_count: 0 }),
                    _ => {
                        return Err(Error::Internal(
                            "container resource has drifted out of sync from a started state: cannot recover".to_string(),
                        ))
                    }
                },
                Err(DockerError::DockerResponseServerError {
                    status_code: 404, ..
                }) => {
                    // container not found, let's try to recreate it
                    // with the same image
                    Self::Creating(ServiceCreating::from_container(container, 0)?)
                }
                Err(err) => return Err(Error::Docker(err)),
            },
            Self::Stopping(ServiceStopping { container })
             => match container
                .clone()
                .refresh(ctx)
                .await
            {
                Ok(container) => match safe_unwrap!(container.state.status) {
                    ContainerStateStatusEnum::RUNNING => {
                        Self::Stopping(ServiceStopping{ container })
                    }
                    ContainerStateStatusEnum::EXITED => Self::Stopped(ServiceStopped { container }),
                    _ => {
                        return Err(Error::Internal(
                            "container resource has drifted out of sync from a stopping state: cannot recover".to_string(),
                        ))
                    }
                },
                Err(err) => return Err(Error::Docker(err)),
            },
            Self::Restarting(restarting) => Self::Restarting(restarting),
            Self::Recreating(recreating) => Self::Recreating(recreating),
            Self::Stopped(stopped) => Self::Stopped(stopped),
            Self::Rebooting(rebooting) => Self::Rebooting(rebooting),
            Self::Destroying(destroying) => Self::Destroying(destroying),
            Self::Destroyed(destroyed) => Self::Destroyed(destroyed),
            Self::Errored(err) => Self::Errored(err),
        };
        Ok(refreshed)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HealthCheckRecord {
    at: chrono::DateTime<chrono::Utc>,
    is_healthy: bool,
}

impl HealthCheckRecord {
    pub fn new(is_healthy: bool) -> Self {
        Self {
            at: chrono::Utc::now(),
            is_healthy,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Service {
    id: String,
    target: Ipv4Addr,
    last_check: Option<HealthCheckRecord>,
}

impl Service {
    pub fn from_container(container: ContainerInspectResponse) -> Result<Self, ServiceErrored> {
        let service_id = container.service_id()?;

        let network = safe_unwrap!(container.network_settings.networks)
            .values()
            .next()
            .ok_or_else(|| ServiceErrored::internal("project was not linked to a network"))?;

        let target = safe_unwrap!(network.ip_address)
            .parse()
            .map_err(|_| ServiceErrored::internal("project did not join the network"))?;

        Ok(Self {
            id: service_id.to_string(),
            target,
            last_check: None,
        })
    }

    pub async fn is_healthy(
        &mut self,
        runtime_manager: RuntimeManager,
    ) -> Result<bool, error::Error> {
        let service_id = Ulid::from_string(self.id.as_str()).map_err(error::Error::Decode)?;
        let is_healthy = runtime_manager.is_healthy(&service_id).await;
        self.last_check = Some(HealthCheckRecord::new(is_healthy));
        Ok(is_healthy)
    }
}
