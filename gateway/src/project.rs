use std::convert::Infallible;
use std::fmt::Formatter;
use std::net::{IpAddr, Ipv4Addr};
use std::time::Duration;

use bollard::container::{Config, CreateContainerOptions, StopContainerOptions, RemoveContainerOptions};
use bollard::errors::Error as DockerError;
use bollard::models::{
    ContainerConfig, ContainerInspectResponse, ContainerState, ContainerStateStatusEnum,
    HealthStatusEnum, HostConfig, Mount, MountTypeEnum, NetworkingConfig,
};
use futures::prelude::*;
use rand::distributions::{Alphanumeric, DistString};
use serde::{Deserialize, Serialize};
use tokio::time;

use crate::args::Args;

use super::{Context, EndState, Error, ErrorKind, IntoEndState, ProjectName, Refresh, State};

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

macro_rules! safe_unwrap_mut {
    {$fst:ident$(.$attr:ident$(($ex:expr))?)+} => {
        $fst$(
            .$attr$(($ex))?
                .as_mut()
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

#[async_trait]
impl Refresh for ContainerInspectResponse {
    type Error = DockerError;
    async fn refresh<'c, C: Context<'c>>(self, ctx: &C) -> Result<Self, Self::Error> {
        ctx.docker()
            .inspect_container(self.id.as_ref().unwrap(), None)
            .await
    }
}

impl From<DockerError> for Error {
    fn from(err: DockerError) -> Self {
        error!("internal Docker error: {err}");
        Self::source(ErrorKind::Internal, err)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Project {
    Creating(ProjectCreating),
    Ready(ProjectReady),
    Started(ProjectStarted),
    Stopped(ProjectStopped),
    Errored(ProjectError),
}

impl_from_variant!(Project:
                   ProjectCreating => Creating,
                   ProjectReady => Ready,
                   ProjectStarted => Started,
                   ProjectStopped => Stopped,
                   ProjectError => Errored);

impl Project {
    pub async fn stop<'c, C: Context<'c>>(self, ctx: &C) -> Result<Self, Error> {
        match self {
            Self::Creating(_) => Err(Error::custom(
                ErrorKind::InvalidOperation,
                "tried to stop a project that was not ready",
            )),
            Self::Ready(ProjectReady { container, .. }) => {
                Ok(Self::Stopped(ProjectStopped { container }))
            }
            Self::Started(ProjectStarted { container, .. }) => {
                ctx.docker()
                    .stop_container(container.id.as_ref().unwrap(), None)
                    .await?;
                Ok(Self::Stopped(ProjectStopped {
                    container: container.refresh(ctx).await?,
                }))
            }
            Self::Stopped(stopped) => Ok(Self::Stopped(stopped)),
            Self::Errored(err) => Ok(Self::Errored(err)),
        }
    }

    pub fn target_ip(&self) -> Result<Option<String>, Error> {
        match self.clone() {
            Self::Started(project_started) => Ok(Some(project_started.target_ip().to_string())),
            _ => Ok(None), // not ready
        }
    }

    pub fn state(&self) -> &'static str {
        match self {
            Self::Started(_) => "started",
            Self::Stopped(_) => "stopped",
            Self::Ready(_) => "ready",
            Self::Creating(_) => "creating",
            Self::Errored(_) => "error",
        }
    }

    pub async fn destroy<'c, C: Context<'c>>(self, ctx: &C) -> Result<(), Error> {
        match self {
            Self::Ready(ProjectReady {
                container: ContainerInspectResponse { id, .. },
                ..
            })
            | Self::Started(ProjectStarted {
                container: ContainerInspectResponse { id, .. },
                ..
            })
            | Self::Stopped(ProjectStopped {
                container: ContainerInspectResponse { id, .. },
            }) => {
                let container_id = id.as_ref().unwrap();
                ctx.docker()
                    .stop_container(container_id, Some(StopContainerOptions { t: 30 }))
                    .await
                    .unwrap_or(());
                ctx.docker()
                    .remove_container(container_id, Some(RemoveContainerOptions {
                        force: true,
                        ..Default::default()
                    }))
                    .await
                    .unwrap_or(());
                Ok(())
            }
            Self::Creating(_) | Self::Errored(_) => Ok(()),
        }
    }
}

#[async_trait]
impl<'c> State<'c> for Project {
    type Next = Self;
    type Error = Infallible;

    async fn next<C: Context<'c>>(self, ctx: &C) -> Result<Self::Next, Self::Error> {
        match self {
            Self::Creating(creating) => creating.next(ctx).await.into_end_state(),
            Self::Ready(ready) => ready.next(ctx).await.into_end_state(),
            Self::Started(started) => started.next(ctx).await.into_end_state(),
            Self::Stopped(stopped) => stopped.next(ctx).await.into_end_state(),
            Self::Errored(errored) => Ok(Self::Errored(errored)),
        }
    }
}

impl<'c> EndState<'c> for Project {
    fn is_done(&self) -> bool {
        matches!(self, Self::Errored(_) | Self::Started(_))
    }
}

#[async_trait]
impl Refresh for Project {
    type Error = Error;

    /// TODO: we could be a bit more clever than this by using the
    /// health checks instead of matching against the raw container
    /// state which is probably prone to erroneously setting the
    /// project into the wrong state if the docker is transitioning
    /// the state of its resources under us
    async fn refresh<'c, C: Context<'c>>(self, ctx: &C) -> Result<Self, Self::Error> {
        let next = match self {
            Self::Creating(creating) => Self::Creating(creating),
            Self::Ready(ProjectReady { container })
            | Self::Started(ProjectStarted { container, .. })
            | Self::Stopped(ProjectStopped { container }) => {
                let container_name = container.name.as_ref().unwrap().to_owned();
                match container.refresh(ctx).await {
                    Ok(container) => {
                        match container.state.as_ref().unwrap().status.as_ref().unwrap() {
                            ContainerStateStatusEnum::RUNNING => {
                                let service = Service::from_container(container.clone(), ctx)?;
                                Self::Started(ProjectStarted { container, service })
                            }
                            ContainerStateStatusEnum::CREATED => {
                                Self::Ready(ProjectReady { container })
                            }
                            ContainerStateStatusEnum::EXITED => {
                                Self::Stopped(ProjectStopped { container })
                            }
                            _ => todo!(),
                        }
                    }
                    Err(_err) => todo!(),
                }
            }
            Self::Errored(err) => Self::Errored(err),
        };
        Ok(next)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProjectCreating {
    project_name: ProjectName,
    initial_key: String,
}

impl ProjectCreating {
    pub fn new(project_name: ProjectName, prefix: String, initial_key: String) -> Self {
        Self {
            project_name,
            initial_key,
        }
    }

    fn container_name<'c, C: Context<'c>>(&self, ctx: &C) -> String {
        let Args { prefix, .. } = &ctx.args();

        let Self { project_name, .. } = &self;

        format!("{prefix}{project_name}_run")
    }

    fn generate_container_config<'c, C: Context<'c>>(
        &self,
        ctx: &C
    ) -> (CreateContainerOptions<String>, Config<String>) {
        let pg_password = Alphanumeric.sample_string(&mut rand::thread_rng(), 12);

        let Args { image, prefix, provisioner_host, network_id, .. } = &ctx.args();

        let Self { initial_key, project_name, .. } = &self;

        let create_container_options = CreateContainerOptions {
            name: self.container_name(ctx)
        };

        let container_config: ContainerConfig = deserialize_json!({
            "Image": image,
            "Hostname": format!("{prefix}{project_name}"),
            "Env": [
                "PROXY_PORT=8000",
                "API_PORT=8001",
                "PG_PORT=5432",
                "PG_DATA=/opt/shuttle/postgres",
                format!("PG_PASSWORD={pg_password}"),
                format!("SHUTTLE_INITIAL_KEY={initial_key}"),
                format!("PROVISIONER_ADDRESS={provisioner_host}"),
                "SHUTTLE_USERS_TOML=/opt/shuttle/users.toml",
                "COPY_PG_CONF=/opt/shuttle/conf/postgres",
                "PROXY_FQDN=shuttleapp.rs"
            ],
            "Labels": {
                "shuttle_prefix": prefix
            }
        });

        let mut config = Config::<String>::from(container_config);

        config.networking_config = deserialize_json!({
            "EndpointsConfig": {
                self.container_name(ctx): {
                    "NetworkID": network_id
                }
            }
        });

        config.host_config = deserialize_json!({
            "Mounts": [{
                "Target": "/opt/shuttle",
                "Source": format!("{prefix}{project_name}_vol"),
                "Type": "volume"
            }]
        });

        debug!(r"generated a container configuration:
CreateContainerOpts: {create_container_options:#?}
Config: {config:#?}
");

        (create_container_options, config)
    }
}

#[async_trait]
impl<'c> State<'c> for ProjectCreating {
    type Next = ProjectReady;
    type Error = ProjectError;

    async fn next<C: Context<'c>>(self, ctx: &C) -> Result<Self::Next, Self::Error> {
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
        Ok(ProjectReady { container })
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProjectReady {
    container: ContainerInspectResponse,
}

#[async_trait]
impl<'c> State<'c> for ProjectReady {
    type Next = ProjectStarted;
    type Error = ProjectError;

    async fn next<C: Context<'c>>(self, ctx: &C) -> Result<Self::Next, Self::Error> {
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

        let mut container = None;
        for _ in 0..9 {
            let latest = self.container.clone().refresh(ctx).await?;
            if matches!(
                safe_unwrap!(latest.state.health.status),
                HealthStatusEnum::HEALTHY
            ) {
                container = Some(latest);
                break;
            } else {
                time::sleep(Duration::from_secs(1)).await;
            }
        }

        if let Some(container) = container {
            let service = Service::from_container(container.clone(), ctx)?;
            Ok(Self::Next { container, service })
        } else {
            Err(ProjectError::internal(
                "timed out waiting for runtime to become healthy",
            ))
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProjectStarted {
    container: ContainerInspectResponse,
    service: Service,
}

impl ProjectStarted {
    pub fn name(&self) -> &str {
        &self.service.name
    }

    pub fn target_ip(&self) -> &IpAddr {
        &self.service.target
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Service {
    name: String,
    target: IpAddr,
}

impl Service {
    pub fn from_container<'c, C: Context<'c>>(
        mut container: ContainerInspectResponse,
        ctx: &C,
    ) -> Result<Self, ProjectError> {
        let container_name = safe_unwrap!(container.name.strip_prefix("/"))
            .to_string();

        let resource_name = safe_unwrap!(container_name.strip_suffix("_run"))
            .to_string();

        let Args { network_id, .. } = ctx.args();

        let target = safe_unwrap_mut!(
            container
                .network_settings
                .networks
                .remove(&container_name)
                .ip_address
        )
            .parse()
            .unwrap();

        Ok(Self { name: resource_name, target })
    }
}

#[async_trait]
impl Refresh for ProjectStarted {
    type Error = Error;

    async fn refresh<'c, C: Context<'c>>(self, ctx: &C) -> Result<Self, Self::Error> {
        let container = self.container.refresh(ctx).await?;
        let service = Service::from_container(container.clone(), ctx)?;
        Ok(Self { container, service })
    }
}

#[async_trait]
impl<'c> State<'c> for ProjectStarted {
    type Next = Self;
    type Error = ProjectError;

    async fn next<C: Context<'c>>(self, ctx: &C) -> Result<Self::Next, Self::Error> {
        Ok(self)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProjectStopped {
    container: ContainerInspectResponse,
}

#[async_trait]
impl Refresh for ProjectStopped {
    type Error = Error;

    async fn refresh<'c, C: Context<'c>>(self, ctx: &C) -> Result<Self, Self::Error> {
        Ok(Self {
            container: self.container.refresh(ctx).await?,
        })
    }
}

#[async_trait]
impl<'c> State<'c> for ProjectStopped {
    type Next = ProjectReady;
    type Error = ProjectError;

    async fn next<C: Context<'c>>(self, ctx: &C) -> Result<Self::Next, Self::Error> {
        // If stopped, try to restart
        Ok(ProjectReady {
            container: self.container,
        })
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ProjectErrorKind {
    Internal,
}

/// A runtime error coming from inside a project
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProjectError {
    kind: ProjectErrorKind,
    message: String,
}

impl ProjectError {
    pub fn internal<S: AsRef<str>>(message: S) -> Self {
        Self {
            kind: ProjectErrorKind::Internal,
            message: message.as_ref().to_string(),
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
        error!("an internal DockerError had to yield a ProjectError: {err}");
        Self {
            kind: ProjectErrorKind::Internal,
            message: format!("{}", err),
        }
    }
}

impl From<ProjectError> for Error {
    fn from(err: ProjectError) -> Self {
        Self::source(ErrorKind::Internal, err)
    }
}

#[async_trait]
impl<'c> State<'c> for ProjectError {
    type Next = Self;
    type Error = Infallible;

    async fn next<C: Context<'c>>(self, ctx: &C) -> Result<Self::Next, Self::Error> {
        Ok(self)
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::World;

    #[tokio::test]
    async fn create_project() {
        let world = World::new();
        let ctx = world.context();
        let mut project = Project::Creating(ProjectCreating::new(
            "test_project_do_not_upvote".parse().unwrap(),
        ));
        while !matches!(&project, Project::Started(..)) {
            project = project.next(&ctx).await.unwrap();
        }
        project = project.stop(&ctx).await.unwrap();
        assert!(matches!(project, Project::Stopped(_)));
        project.destroy(&ctx).await.unwrap();
    }
}
