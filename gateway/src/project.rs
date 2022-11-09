use std::collections::HashMap;
use std::convert::Infallible;
use std::net::{IpAddr, SocketAddr};
use std::time::Duration;

use bollard::container::{
    Config, CreateContainerOptions, RemoveContainerOptions, StopContainerOptions,
};
use bollard::errors::Error as DockerError;
use bollard::models::{ContainerConfig, ContainerInspectResponse, ContainerStateStatusEnum};
use bollard::system::EventsOptions;
use futures::prelude::*;
use http::uri::InvalidUri;
use http::Uri;
use hyper::client::HttpConnector;
use hyper::Client;
use once_cell::sync::Lazy;
use rand::distributions::{Alphanumeric, DistString};
use serde::{Deserialize, Serialize};
use tokio::time;
use tracing::{debug, error};

use crate::{
    ContainerSettings, DockerContext, EndState, Error, ErrorKind, IntoTryState, ProjectName,
    Refresh, State, TryState,
};

macro_rules! safe_unwrap {
    {$fst:ident$(.$attr:ident$(($ex:expr))?)+} => {
        $fst$(
            .$attr$(($ex))?
                .as_ref()
                .ok_or_else(|| ProjectError::internal(
                    concat!("container state object is malformed at attribute: ", stringify!($attr))
                ))?
        )+
    }
}

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

const RUNTIME_API_PORT: u16 = 8001;
const MAX_RESTARTS: usize = 3;

// Client used for health checks
static CLIENT: Lazy<Client<HttpConnector>> = Lazy::new(Client::new);

#[async_trait]
impl<Ctx> Refresh<Ctx> for ContainerInspectResponse
where
    Ctx: DockerContext,
{
    type Error = DockerError;
    async fn refresh(self, ctx: &Ctx) -> Result<Self, Self::Error> {
        ctx.docker()
            .inspect_container(self.id.as_ref().unwrap(), None)
            .await
    }
}

impl From<DockerError> for Error {
    fn from(err: DockerError) -> Self {
        error!(error = %err, "internal Docker error");
        Self::source(ErrorKind::Internal, err)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Project {
    Creating(ProjectCreating),
    Starting(ProjectStarting),
    Started(ProjectStarted),
    Ready(ProjectReady),
    Stopping(ProjectStopping),
    Stopped(ProjectStopped),
    Destroying(ProjectDestroying),
    Destroyed(ProjectDestroyed),
    Errored(ProjectError),
}

impl_from_variant!(Project:
                   ProjectCreating => Creating,
                   ProjectStarting => Starting,
                   ProjectStarted => Started,
                   ProjectReady => Ready,
                   ProjectStopping => Stopping,
                   ProjectStopped => Stopped,
                   ProjectDestroying => Destroying,
                   ProjectDestroyed => Destroyed,
                   ProjectError => Errored);

impl Project {
    pub fn stop(self) -> Result<Self, Error> {
        if let Some(container) = self.container() {
            Ok(Self::Stopping(ProjectStopping { container }))
        } else {
            Err(Error::custom(
                ErrorKind::InvalidOperation,
                format!("cannot stop a project in the `{}` state", self.state()),
            ))
        }
    }

    pub fn create(project_name: ProjectName) -> Self {
        Self::Creating(ProjectCreating::new_with_random_initial_key(project_name))
    }

    pub fn destroy(self) -> Result<Self, Error> {
        if let Some(container) = self.container() {
            Ok(Self::Destroying(ProjectDestroying { container }))
        } else {
            Ok(Self::Destroyed(ProjectDestroyed { destroyed: None }))
        }
    }

    pub fn is_ready(&self) -> bool {
        matches!(self, Self::Ready(_))
    }

    pub fn is_destroyed(&self) -> bool {
        matches!(self, Self::Destroyed(_))
    }

    pub fn target_ip(&self) -> Result<Option<IpAddr>, Error> {
        match self.clone() {
            Self::Ready(project_ready) => Ok(Some(*project_ready.target_ip())),
            _ => Ok(None), // not ready
        }
    }

    pub fn target_addr(&self) -> Result<Option<SocketAddr>, Error> {
        Ok(self
            .target_ip()?
            .map(|target_ip| SocketAddr::new(target_ip, RUNTIME_API_PORT)))
    }

    pub fn state(&self) -> &'static str {
        match self {
            Self::Started(_) => "started",
            Self::Ready(_) => "ready",
            Self::Stopped(_) => "stopped",
            Self::Starting(_) => "starting",
            Self::Stopping(_) => "stopping",
            Self::Creating(_) => "creating",
            Self::Destroying(_) => "destroying",
            Self::Destroyed(_) => "destroyed",
            Self::Errored(_) => "error",
        }
    }

    pub fn container(&self) -> Option<ContainerInspectResponse> {
        match self {
            Self::Starting(ProjectStarting { container, .. })
            | Self::Started(ProjectStarted { container, .. })
            | Self::Ready(ProjectReady { container, .. })
            | Self::Stopping(ProjectStopping { container })
            | Self::Stopped(ProjectStopped { container })
            | Self::Destroying(ProjectDestroying { container }) => Some(container.clone()),
            Self::Errored(ProjectError { ctx: Some(ctx), .. }) => ctx.container(),
            Self::Errored(_) | Self::Creating(_) | Self::Destroyed(_) => None,
        }
    }

    pub fn initial_key(&self) -> Option<&String> {
        if let Self::Creating(ProjectCreating { initial_key, .. }) = self {
            Some(initial_key)
        } else {
            None
        }
    }

    pub fn container_id(&self) -> Option<String> {
        self.container().and_then(|container| container.id)
    }
}

impl From<Project> for shuttle_common::models::project::State {
    fn from(project: Project) -> Self {
        match project {
            Project::Creating(_) => Self::Creating,
            Project::Starting(_) => Self::Starting,
            Project::Started(_) => Self::Started,
            Project::Ready(_) => Self::Ready,
            Project::Stopping(_) => Self::Stopping,
            Project::Stopped(_) => Self::Stopped,
            Project::Destroying(_) => Self::Destroying,
            Project::Destroyed(_) => Self::Destroyed,
            Project::Errored(_) => Self::Errored,
        }
    }
}

#[async_trait]
impl<Ctx> State<Ctx> for Project
where
    Ctx: DockerContext,
{
    type Next = Self;
    type Error = Infallible;

    async fn next(self, ctx: &Ctx) -> Result<Self::Next, Self::Error> {
        let previous = self.clone();
        let previous_state = previous.state();

        let mut new = match self {
            Self::Creating(creating) => creating.next(ctx).await.into_try_state(),
            Self::Starting(ready) => ready.next(ctx).await.into_try_state(),
            Self::Started(started) => match started.next(ctx).await {
                Ok(ProjectReadying::Ready(ready)) => Ok(ready.into()),
                Ok(ProjectReadying::Started(started)) => Ok(started.into()),
                Err(err) => Ok(Self::Errored(err)),
            },
            Self::Ready(ready) => ready.next(ctx).await.into_try_state(),
            Self::Stopped(stopped) => stopped.next(ctx).await.into_try_state(),
            Self::Stopping(stopping) => stopping.next(ctx).await.into_try_state(),
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

impl<Ctx> EndState<Ctx> for Project
where
    Ctx: DockerContext,
{
    fn is_done(&self) -> bool {
        matches!(self, Self::Errored(_) | Self::Ready(_) | Self::Destroyed(_))
    }
}

impl TryState for Project {
    type ErrorVariant = ProjectError;

    fn into_result(self) -> Result<Self, Self::ErrorVariant> {
        match self {
            Self::Errored(perr) => Err(perr),
            otherwise => Ok(otherwise),
        }
    }
}

#[async_trait]
impl<Ctx> Refresh<Ctx> for Project
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
        let _container = if let Some(container_id) = self.container_id() {
            Some(ctx.docker().inspect_container(&container_id, None).await?)
        } else {
            None
        };

        let refreshed = match self {
            Self::Creating(creating) => Self::Creating(creating),
            Self::Starting(ProjectStarting { container })
            | Self::Started(ProjectStarted { container, .. })
            | Self::Ready(ProjectReady { container, .. })
            | Self::Stopping(ProjectStopping { container })
            | Self::Stopped(ProjectStopped { container }) => {
                let container = container.refresh(ctx).await?;
                match container.state.as_ref().unwrap().status.as_ref().unwrap() {
                    ContainerStateStatusEnum::RUNNING => {
                        let service = Service::from_container(container.clone())?;
                        Self::Started(ProjectStarted { container, service })
                    }
                    ContainerStateStatusEnum::CREATED => {
                        Self::Starting(ProjectStarting { container })
                    }
                    ContainerStateStatusEnum::EXITED => Self::Stopped(ProjectStopped { container }),
                    _ => {
                        return Err(Error::custom(
                            ErrorKind::Internal,
                            "container resource has drifted out of sync: cannot recover",
                        ))
                    }
                }
            }
            Self::Destroying(destroying) => Self::Destroying(destroying),
            Self::Destroyed(destroyed) => Self::Destroyed(destroyed),
            Self::Errored(err) => Self::Errored(err),
        };
        Ok(refreshed)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectCreating {
    project_name: ProjectName,
    initial_key: String,
}

impl ProjectCreating {
    pub fn new(project_name: ProjectName, initial_key: String) -> Self {
        Self {
            project_name,
            initial_key,
        }
    }

    pub fn new_with_random_initial_key(project_name: ProjectName) -> Self {
        let initial_key = Alphanumeric.sample_string(&mut rand::thread_rng(), 32);
        Self::new(project_name, initial_key)
    }

    pub fn project_name(&self) -> &ProjectName {
        &self.project_name
    }

    fn container_name<C: DockerContext>(&self, ctx: &C) -> String {
        let prefix = &ctx.container_settings().prefix;

        let Self { project_name, .. } = &self;

        format!("{prefix}{project_name}_run")
    }

    fn generate_container_config<C: DockerContext>(
        &self,
        ctx: &C,
    ) -> (CreateContainerOptions<String>, Config<String>) {
        let ContainerSettings {
            image,
            prefix,
            provisioner_host,
            network_name,
            network_id,
            fqdn,
            ..
        } = ctx.container_settings();

        let Self {
            initial_key,
            project_name,
            ..
        } = &self;

        let create_container_options = CreateContainerOptions {
            name: self.container_name(ctx),
        };

        let container_config: ContainerConfig = deserialize_json!({
            "Image": image,
            "Hostname": format!("{prefix}{project_name}"),
            "Labels": {
                "shuttle_prefix": prefix
            },
            "Cmd": [
                "--admin-secret",
                initial_key,
                "--api-address",
                format!("0.0.0.0:{RUNTIME_API_PORT}"),
                "--provisioner-address",
                provisioner_host,
                "--provisioner-port",
                "8000",
                "--proxy-address",
                "0.0.0.0:8000",
                "--proxy-fqdn",
                fqdn,
                "--artifacts-path",
                "/opt/shuttle",
                "--state",
                "/opt/shuttle/deployer.sqlite",
            ],
            "Env": [
                "RUST_LOG=debug",
            ],
            "Labels": {
                "project.name": project_name,
            }
        });

        let mut config = Config::<String>::from(container_config);

        config.networking_config = deserialize_json!({
            "EndpointsConfig": {
                network_name: {
                    "NetworkID": network_id
                }
            }
        });

        config.host_config = deserialize_json!({
            "Mounts": [{
                "Target": "/opt/shuttle",
                "Source": format!("{prefix}{project_name}_vol"),
                "Type": "volume"
            }],
            "Memory": 6442450000i64, // 6 GiB hard limit
            "MemoryReservation": 4295000000i64, // 4 GiB soft limit, applied if host is low on memory
        });

        debug!(
            r"generated a container configuration:
CreateContainerOpts: {create_container_options:#?}
Config: {config:#?}
"
        );

        (create_container_options, config)
    }
}

#[async_trait]
impl<Ctx> State<Ctx> for ProjectCreating
where
    Ctx: DockerContext,
{
    type Next = ProjectStarting;
    type Error = ProjectError;

    async fn next(self, ctx: &Ctx) -> Result<Self::Next, Self::Error> {
        let container_name = self.container_name(ctx);
        let container = ctx
            .docker()
            // If container already exists, use that
            .inspect_container(&container_name.clone(), None)
            // Otherwise create it
            .or_else(|err| async move {
                if matches!(err, DockerError::DockerResponseServerError { status_code, .. } if status_code == 404) {
                    let (opts, config) = self.generate_container_config(ctx);
                    ctx.docker()
                        .create_container(Some(opts), config)
                        .and_then(|_| ctx.docker().inspect_container(&container_name, None))
                        .await
                } else {
                    Err(err)
                }
            })
            .await?;
        Ok(ProjectStarting { container })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProjectStarting {
    container: ContainerInspectResponse,
}

#[async_trait]
impl<Ctx> State<Ctx> for ProjectStarting
where
    Ctx: DockerContext,
{
    type Next = ProjectStarted;
    type Error = ProjectError;

    async fn next(self, ctx: &Ctx) -> Result<Self::Next, Self::Error> {
        let container_id = self.container.id.as_ref().unwrap();
        ctx.docker()
            .start_container::<String>(container_id, None)
            .await
            .or_else(|err| {
                if matches!(err, DockerError::DockerResponseServerError { status_code, .. } if status_code == 304) {
                    // Already started
                    Ok(())
                } else {
                    Err(err)
                }
            })?;

        let container = self.container.refresh(ctx).await?;

        let service = Service::from_container(container.clone())?;

        Ok(Self::Next { container, service })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProjectStarted {
    container: ContainerInspectResponse,
    service: Service,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ProjectReadying {
    Ready(ProjectReady),
    Started(ProjectStarted),
}

#[async_trait]
impl<Ctx> State<Ctx> for ProjectStarted
where
    Ctx: DockerContext,
{
    type Next = ProjectReadying;
    type Error = ProjectError;

    async fn next(self, ctx: &Ctx) -> Result<Self::Next, Self::Error> {
        time::sleep(Duration::from_secs(1)).await;

        let container = self.container.refresh(ctx).await?;
        let mut service = self.service;

        if service.is_healthy().await {
            Ok(Self::Next::Ready(ProjectReady { container, service }))
        } else {
            let started_at =
                chrono::DateTime::parse_from_rfc3339(safe_unwrap!(container.state.started_at))
                    .map_err(|_err| {
                        ProjectError::internal("invalid `started_at` response from Docker daemon")
                    })?;
            let now = chrono::offset::Utc::now();
            if started_at + chrono::Duration::seconds(120) < now {
                return Err(ProjectError::internal(
                    "project did not become healthy in time",
                ));
            }

            Ok(Self::Next::Started(ProjectStarted { container, service }))
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProjectReady {
    container: ContainerInspectResponse,
    service: Service,
}

#[async_trait]
impl<Ctx> State<Ctx> for ProjectReady
where
    Ctx: DockerContext,
{
    type Next = Self;
    type Error = ProjectError;

    async fn next(mut self, _ctx: &Ctx) -> Result<Self::Next, Self::Error> {
        Ok(self)
    }
}

impl ProjectReady {
    pub fn name(&self) -> &str {
        &self.service.name
    }

    pub fn target_ip(&self) -> &IpAddr {
        &self.service.target
    }

    pub async fn is_healthy(&mut self) -> bool {
        self.service.is_healthy().await
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
    name: String,
    target: IpAddr,
    last_check: Option<HealthCheckRecord>,
}

impl Service {
    pub fn from_container(container: ContainerInspectResponse) -> Result<Self, ProjectError> {
        let container_name = safe_unwrap!(container.name.strip_prefix("/")).to_string();

        let resource_name = safe_unwrap!(container_name.strip_suffix("_run")).to_string();

        let network = safe_unwrap!(container.network_settings.networks)
            .values()
            .next()
            .ok_or_else(|| ProjectError::internal("project was not linked to a network"))?;
        let target = safe_unwrap!(network.ip_address).parse().unwrap();

        Ok(Self {
            name: resource_name,
            target,
            last_check: None,
        })
    }

    pub fn uri<S: AsRef<str>>(&self, path: S) -> Result<Uri, ProjectError> {
        format!("http://{}:8001{}", self.target, path.as_ref())
            .parse::<Uri>()
            .map_err(|err| err.into())
    }

    pub async fn is_healthy(&mut self) -> bool {
        let uri = self.uri(format!("/projects/{}/status", self.name)).unwrap();
        let is_healthy = matches!(CLIENT.get(uri).await, Ok(res) if res.status().is_success());
        self.last_check = Some(HealthCheckRecord::new(is_healthy));
        is_healthy
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProjectStopping {
    container: ContainerInspectResponse,
}

#[async_trait]
impl<Ctx> State<Ctx> for ProjectStopping
where
    Ctx: DockerContext,
{
    type Next = ProjectStopped;

    type Error = ProjectError;

    async fn next(self, ctx: &Ctx) -> Result<Self::Next, Self::Error> {
        let Self { container } = self;
        ctx.docker()
            .stop_container(
                container.id.as_ref().unwrap(),
                Some(StopContainerOptions { t: 30 }),
            )
            .await?;
        Ok(Self::Next {
            container: container.refresh(ctx).await?,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProjectStopped {
    container: ContainerInspectResponse,
}

#[async_trait]
impl<Ctx> State<Ctx> for ProjectStopped
where
    Ctx: DockerContext,
{
    type Next = ProjectStarting;
    type Error = ProjectError;

    async fn next(self, ctx: &Ctx) -> Result<Self::Next, Self::Error> {
        let container = self.container;

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

        let start_event_count = start_events.iter().count();
        debug!(
            "project started {} times in the last 15 minutes",
            start_event_count
        );

        // If stopped, and has not restarted too much, try to restart
        if start_event_count < MAX_RESTARTS {
            Ok(ProjectStarting { container })
        } else {
            Err(ProjectError::internal(
                "too many restarts in the last 15 minutes",
            ))
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProjectDestroying {
    container: ContainerInspectResponse,
}

#[async_trait]
impl<Ctx> State<Ctx> for ProjectDestroying
where
    Ctx: DockerContext,
{
    type Next = ProjectDestroyed;
    type Error = ProjectError;

    async fn next(self, ctx: &Ctx) -> Result<Self::Next, Self::Error> {
        let container_id = self.container.id.as_ref().unwrap();
        ctx.docker()
            .stop_container(container_id, Some(StopContainerOptions { t: 1 }))
            .await
            .unwrap_or(());
        ctx.docker()
            .remove_container(
                container_id,
                Some(RemoveContainerOptions {
                    force: true,
                    ..Default::default()
                }),
            )
            .await
            .unwrap_or(());
        Ok(Self::Next {
            destroyed: Some(self.container),
        })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProjectDestroyed {
    destroyed: Option<ContainerInspectResponse>,
}

#[async_trait]
impl<Ctx> State<Ctx> for ProjectDestroyed
where
    Ctx: DockerContext,
{
    type Next = ProjectDestroyed;
    type Error = ProjectError;

    async fn next(self, _ctx: &Ctx) -> Result<Self::Next, Self::Error> {
        Ok(self)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProjectErrorKind {
    Internal,
}

/// A runtime error coming from inside a project
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProjectError {
    kind: ProjectErrorKind,
    message: String,
    ctx: Option<Box<Project>>,
}

impl ProjectError {
    pub fn internal<S: AsRef<str>>(message: S) -> Self {
        Self {
            kind: ProjectErrorKind::Internal,
            message: message.as_ref().to_string(),
            ctx: None,
        }
    }
}

impl std::fmt::Display for ProjectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for ProjectError {}

impl From<DockerError> for ProjectError {
    fn from(err: DockerError) -> Self {
        error!(error = %err, "an internal DockerError had to yield a ProjectError");
        Self {
            kind: ProjectErrorKind::Internal,
            message: format!("{}", err),
            ctx: None,
        }
    }
}

impl From<InvalidUri> for ProjectError {
    fn from(uri: InvalidUri) -> Self {
        error!(%uri, "failed to create a health check URI");

        Self {
            kind: ProjectErrorKind::Internal,
            message: uri.to_string(),
            ctx: None,
        }
    }
}

impl From<hyper::Error> for ProjectError {
    fn from(err: hyper::Error) -> Self {
        error!(error = %err, "failed to check project's health");

        Self {
            kind: ProjectErrorKind::Internal,
            message: err.to_string(),
            ctx: None,
        }
    }
}

impl From<ProjectError> for Error {
    fn from(err: ProjectError) -> Self {
        Self::source(ErrorKind::Internal, err)
    }
}

#[async_trait]
impl<Ctx> State<Ctx> for ProjectError
where
    Ctx: DockerContext,
{
    type Next = Self;
    type Error = Infallible;

    async fn next(self, _ctx: &Ctx) -> Result<Self::Next, Self::Error> {
        Ok(self)
    }
}

pub mod exec {

    use std::sync::Arc;

    use bollard::service::ContainerState;
    use tokio::sync::mpsc::Sender;

    use crate::{
        service::GatewayService,
        task::{self, BoxedTask, TaskResult},
    };

    use super::*;

    pub async fn revive(
        gateway: Arc<GatewayService>,
        sender: Sender<BoxedTask>,
    ) -> Result<(), ProjectError> {
        for (project_name, account_name) in gateway
            .iter_projects()
            .await
            .expect("could not list projects")
        {
            if let Project::Errored(ProjectError { ctx: Some(ctx), .. }) =
                gateway.find_project(&project_name).await.unwrap()
            {
                if let Some(container) = ctx.container() {
                    if let Ok(container) = gateway
                        .context()
                        .docker()
                        .inspect_container(safe_unwrap!(container.id), None)
                        .await
                    {
                        if let Some(ContainerState {
                            status: Some(ContainerStateStatusEnum::EXITED),
                            ..
                        }) = container.state
                        {
                            debug!("{} will be revived", project_name.clone());
                            _ = gateway
                                .new_task()
                                .project(project_name)
                                .account(account_name)
                                .and_then(task::run(|ctx| async move {
                                    TaskResult::Done(Project::Stopped(ProjectStopped {
                                        container: ctx.state.container().unwrap(),
                                    }))
                                }))
                                .send(&sender)
                                .await;
                        }
                    }
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
pub mod tests {

    use bollard::models::ContainerState;
    use futures::prelude::*;
    use hyper::{Body, Request, StatusCode};

    use super::*;
    use crate::tests::{assert_matches, assert_stream_matches, World};
    use crate::EndStateExt;

    #[tokio::test]
    async fn create_start_stop_destroy_project() -> anyhow::Result<()> {
        let world = World::new().await;

        let ctx = world.context();

        let project_started = assert_matches!(
            ctx,
            Project::Creating(ProjectCreating {
                project_name: "my-project-test".parse().unwrap(),
                initial_key: "test".to_string(),
            }),
            #[assertion = "Container created, assigned an `id`"]
            Ok(Project::Starting(ProjectStarting {
                container: ContainerInspectResponse {
                    id: Some(container_id),
                    state: Some(ContainerState {
                        status: Some(ContainerStateStatusEnum::CREATED),
                        ..
                    }),
                    ..
                }
            })),
            #[assertion = "Container started, in a running state"]
            Ok(Project::Started(ProjectStarted {
                container: ContainerInspectResponse {
                    id: Some(id),
                    state: Some(ContainerState {
                        status: Some(ContainerStateStatusEnum::RUNNING),
                        ..
                    }),
                    ..
                },
                ..
            })) if id == container_id,
        );

        let delay = time::sleep(Duration::from_secs(10));
        futures::pin_mut!(delay);
        let mut project_readying = project_started
            .unwrap()
            .into_stream(&ctx)
            .take_until(delay)
            .try_skip_while(|state| future::ready(Ok(!matches!(state, Project::Ready(_)))));

        let project_ready = assert_stream_matches!(
            project_readying,
            #[assertion = "Container is ready"]
            Ok(Project::Ready(ProjectReady {
                container: ContainerInspectResponse {
                    state: Some(ContainerState {
                        status: Some(ContainerStateStatusEnum::RUNNING),
                        ..
                    }),
                    ..
                },
                ..
            })),
        );

        let target_addr = project_ready
            .as_ref()
            .unwrap()
            .target_addr()
            .unwrap()
            .unwrap();

        let client = world.client(target_addr);

        client
            .request(
                Request::get("/projects/my-project-test/status")
                    .body(Body::empty())
                    .unwrap(),
            )
            .map_ok(|resp| assert_eq!(resp.status(), StatusCode::OK))
            .await
            .unwrap();

        let project_stopped = assert_matches!(
            ctx,
            project_ready.unwrap().stop().unwrap(),
            #[assertion = "Container is stopped"]
            Ok(Project::Stopped(ProjectStopped {
                container: ContainerInspectResponse {
                    state: Some(ContainerState {
                        status: Some(ContainerStateStatusEnum::EXITED),
                        ..
                    }),
                    ..
                }
            })),
        );

        assert_matches!(
            ctx,
            project_stopped.unwrap().destroy().unwrap(),
            #[assertion = "Container is destroyed"]
            Ok(Project::Destroyed(ProjectDestroyed { destroyed: _ })),
        )
        .unwrap();

        Ok(())
    }
}
