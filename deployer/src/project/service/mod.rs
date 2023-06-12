use std::{
    fmt::Display,
    net::{IpAddr, SocketAddr},
    time::Duration,
};

use bollard::service::ContainerInspectResponse;
use serde::{Deserialize, Serialize};

use self::{
    error::Error,
    state::{
        attaching::ServiceAttaching, creating::ServiceCreating, destroyed::ServiceDestroyed,
        destroying::ServiceDestroying, errored::ServiceErrored, ready::ServiceReady,
        rebooting::ServiceRebooting, recreating::ServiceRecreating, restarting::ServiceRestarting,
        started::ServiceStarted, starting::ServiceStarting, stopped::ServiceStopped,
        stopping::ServiceStopping,
    },
};

use super::docker::ContainerInspectResponseExt;

pub mod error;
pub mod state;

// Health check must succeed within 10 seconds
static IS_HEALTHY_TIMEOUT: Duration = Duration::from_secs(10);

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

impl Display for ServiceState {
    fn fmt(&self, f: std::fmt::Formatter<'_>) {
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

    pub fn target_ip(&self) -> Result<Option<IpAddr>, Error> {
        match self.clone() {
            Self::Ready(project_ready) => Ok(Some(*project_ready.target_ip())),
            _ => Ok(None), // not ready
        }
    }

    // TODO: pass the shuttle-runtime port
    pub fn target_addr(&self) -> Result<Option<SocketAddr>, Error> {
        Ok(self
            .target_ip()?
            .map(|target_ip| SocketAddr::new(target_ip, RUNTIME_API_PORT)))
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
    target: IpAddr,
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

    // TODO: implement the health-check directed to the shuttle-runtime
    pub fn uri<S: AsRef<str>>(&self, path: S) -> Result<Uri, ProjectError> {
        format!("http://{}:8001{}", self.target, path.as_ref())
            .parse::<Uri>()
            .map_err(|err| err.into())
    }

    // TODO: implement the is_healthy check directed to the shuttle-runtime
    pub async fn is_healthy(&mut self) -> bool {
        let uri = self.uri(format!("/projects/{}/status", self.name)).unwrap();
        let resp = timeout(IS_HEALTHY_TIMEOUT, CLIENT.get(uri)).await;
        let is_healthy = matches!(resp, Ok(Ok(res)) if res.status().is_success());
        self.last_check = Some(HealthCheckRecord::new(is_healthy));
        is_healthy
    }
}
