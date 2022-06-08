use std::fmt::Formatter;
use std::net::{IpAddr, Ipv4Addr};
use std::convert::Infallible;

use bollard::container::{
    Config,
    CreateContainerOptions
};
use bollard::errors::Error as DockerError;
use bollard::models::{
    ContainerInspectResponse,
    ContainerStateStatusEnum,
    HostConfig,
    Mount,
    MountTypeEnum
};
use futures::prelude::*;
use rand::distributions::{Alphanumeric, DistString};
use serde::{
    Deserialize,
    Serialize
};

use crate::ErrorKind;

use super::{
    Context,
    Error,
    ProjectName,
    Refresh,
    State,
    EndState,
    IntoEndState
};

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
        debug!("internal Docker error: {err}");
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

#[derive(Debug)]
pub struct DestroyError {
    error: Error
}

impl std::fmt::Display for DestroyError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.error.fmt(f)
    }
}

impl std::error::Error for DestroyError {}

impl Project {
    pub async fn stop<'c, C: Context<'c>>(self, ctx: &C) -> Result<Self, Error> {
        match self {
            Self::Started(ProjectStarted { container, .. }) => {
                ctx.docker()
                    .stop_container(container.id.as_ref().unwrap(), None)
                    .await
                    .unwrap();
                Ok(Self::Stopped(ProjectStopped {
                    container: container.refresh(ctx).await.unwrap()
                }))
            }
            _otherwise => todo!()
        }
    }

    pub fn target_ip(&self) -> Result<Option<String>, Error> {
        match self.clone() {
            Self::Started(project_started) => {
                Ok(Some(project_started.target_ip().to_string()))
            }
            _ => Ok(None) // not ready
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

    pub async fn destroy<'c, C: Context<'c>>(self, ctx: &C) -> Result<(), DestroyError> {
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
                container: ContainerInspectResponse { id, .. }
            }) => {
                ctx.docker()
                    .remove_container(id.as_ref().unwrap(), None)
                    .await
                    .unwrap();
                Ok(())
            }
            _ => todo!()
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
                                let service = Service::from_container(container.clone(), ctx);
                                Self::Started(ProjectStarted { container, service })
                            }
                            ContainerStateStatusEnum::CREATED => {
                                Self::Ready(ProjectReady { container })
                            }
                            ContainerStateStatusEnum::EXITED => {
                                Self::Stopped(ProjectStopped { container })
                            }
                            _ => todo!()
                        }
                    }
                    Err(_err) => todo!()
                }
            },
            Self::Errored(err) => Self::Errored(err)
        };
        Ok(next)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProjectCreating {
    project_name: ProjectName,
    prefix: String,
    initial_key: String,
}

impl ProjectCreating {
    pub fn new(project_name: ProjectName, prefix: String, initial_key: String) -> Self {
        Self {
            project_name,
            prefix,
            initial_key
        }
    }
}

#[async_trait]
impl<'c> State<'c> for ProjectCreating {
    type Next = ProjectReady;
    type Error = ProjectError;

    async fn next<C: Context<'c>>(self, ctx: &C) -> Result<Self::Next, Self::Error> {
        let pg_password = Alphanumeric.sample_string(&mut rand::thread_rng(), 12);
        let pg_password_env = format!("PG_PASSWORD={}", pg_password);

        let initial_key_env = format!("SHUTTLE_INITIAL_KEY={}", self.initial_key);

        let volume_name = format!("{}{}_vol", self.prefix, self.project_name);
        let container_name = format!("{}{}_run", self.prefix, self.project_name);
        let container = ctx
            .docker()
            .inspect_container(&container_name.clone(), None)
            .or_else(|err| async move {
                if matches!(err, DockerError::DockerResponseNotFoundError { .. }) {
                    let opts = CreateContainerOptions {
                        name: container_name.clone()
                    };
                    let config = Config {
                        image: Some(ctx.args().image.as_str()),
                        env: Some(vec![
                            "PROXY_PORT=8000",
                            "API_PORT=8001",
                            "PG_PORT=5432",
                            "PG_DATA=/opt/shuttle/postgres",
                            &pg_password_env,
                            &initial_key_env,
                            "COPY_PG_CONF=/opt/shuttle/conf/postgres",
                            "PROXY_FQDN=shuttleapp.rs"
                        ]),
                        labels: Some(vec![
                            ("shuttle_prefix", self.prefix.as_str())
                        ].into_iter().collect()),
                        host_config: Some(HostConfig {
                            mounts: Some(vec![Mount {
                                target: Some("/opt/shuttle".to_string()),
                                source: Some(volume_name),
                                typ: Some(MountTypeEnum::VOLUME),
                                ..Default::default()
                            }]),
                            ..Default::default()
                        }),
                        ..Default::default()
                    };
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
    container: ContainerInspectResponse
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
                if matches!(err, DockerError::DockerResponseNotModifiedError { .. }) {
                    // Already started
                    Ok(())
                } else {
                    Err(err)
                }
            })?;
        //let container = self.container.refresh(ctx).await.unwrap();
        let service = Service::from_container(self.container.clone(), ctx);
        Ok(Self::Next {
            container: self.container,
            service
        })
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
    pub fn from_container<'c, C: Context<'c>>(container: ContainerInspectResponse, ctx: &C) -> Self {
        let name = container.name
            .as_ref()
            .unwrap()
            .strip_suffix("_run")
            .unwrap()
            .strip_prefix("/")
            .unwrap()
            .to_string();

        // assumes the container is reachable on a "docker subnet" ip known as "bridge" to docker
        let target = container
            .clone()
            .network_settings
            .unwrap()
            .networks
            .unwrap()
            .remove("bridge")
            .unwrap()
            .ip_address
            .unwrap()
            .parse()
            .unwrap();

        Self {
            name,
            target,
        }
    }
}

#[async_trait]
impl Refresh for ProjectStarted {
    type Error = Error;

    async fn refresh<'c, C: Context<'c>>(self, ctx: &C) -> Result<Self, Self::Error> {
        let container = self.container.refresh(ctx).await.unwrap();
        let service = Service::from_container(container.clone(), ctx);
        Ok(Self {
            container,
            service
        })
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
    container: ContainerInspectResponse
}

#[async_trait]
impl Refresh for ProjectStopped {
    type Error = Error;

    async fn refresh<'c, C: Context<'c>>(self, ctx: &C) -> Result<Self, Self::Error> {
        Ok(Self {
            container: self.container.refresh(ctx).await?
        })
    }
}

#[async_trait]
impl<'c> State<'c> for ProjectStopped {
    type Next = ProjectReady;
    type Error = ProjectError;

    async fn next<C: Context<'c>>(self, ctx: &C) -> Result<Self::Next, Self::Error> {
        // If stopped, try to restart
        Ok(ProjectReady { container: self.container })
    }
}

/// A runtime error coming from inside a project
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProjectError {
    message: String
}

impl std::fmt::Display for ProjectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for ProjectError {}

impl From<DockerError> for ProjectError {
    fn from(err: DockerError) -> Self {
        Self {
            message: format!("{:?}", err)
        }
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
            "test_project_do_not_upvote".parse().unwrap()
        ));
        while !matches!(&project, Project::Started(..)) {
            project = project.next(&ctx).await.unwrap();
        }
        project = project.stop(&ctx).await.unwrap();
        assert!(matches!(project, Project::Stopped(_)));
        project.destroy(&ctx).await.unwrap();
    }
}
