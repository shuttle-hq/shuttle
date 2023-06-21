use tokio::sync::mpsc::Sender;

use async_trait::async_trait;
use bollard::{errors::Error as DockerError, service::ContainerInspectResponse, Docker};
use shuttle_common::models::project::IDLE_MINUTES;
use ulid::Ulid;

use crate::deployment::RunnableDeployment;
use crate::runtime_manager::RuntimeManager;
use crate::safe_unwrap;

use super::service::state::m_errored::ServiceErrored;
use super::service::state::machine::Refresh;

pub struct ContainerSettingsBuilder {
    prefix: Option<String>,
    image_name: Option<String>,
    provisioner: Option<String>,
    auth_uri: Option<String>,
    network_name: Option<String>,
    runtime_start_channel: Option<Sender<RunnableDeployment>>,
    runnable_deployment: Option<RunnableDeployment>,
}

impl Default for ContainerSettingsBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl ContainerSettingsBuilder {
    pub fn new() -> Self {
        Self {
            prefix: None,
            image_name: None,
            provisioner: None,
            auth_uri: None,
            network_name: None,
            runnable_deployment: None,
            runtime_start_channel: None,
        }
    }

    pub fn prefix<S: ToString>(mut self, prefix: S) -> Self {
        self.prefix = Some(prefix.to_string());
        self
    }

    pub fn image<S: ToString>(mut self, image: S) -> Self {
        self.image_name = Some(image.to_string());
        self
    }

    pub fn provisioner_host<S: ToString>(mut self, host: S) -> Self {
        self.provisioner = Some(host.to_string());
        self
    }

    pub fn auth_uri<S: ToString>(mut self, auth_uri: S) -> Self {
        self.auth_uri = Some(auth_uri.to_string());
        self
    }

    pub fn network_name<S: ToString>(mut self, name: S) -> Self {
        self.network_name = Some(name.to_string());
        self
    }

    pub fn runnable_deployment(mut self, runnable_deployment: RunnableDeployment) -> Self {
        self.runnable_deployment = Some(runnable_deployment);
        self
    }

    pub fn runtime_start_channel(mut self, sender: Sender<RunnableDeployment>) -> Self {
        self.runtime_start_channel = Some(sender);
        self
    }

    pub async fn build(mut self) -> ContainerSettings {
        let prefix = self
            .prefix
            .take()
            .expect("to provide a prefix for the container settings");
        let provisioner_host = self
            .provisioner
            .take()
            .expect("to provide a provisioner uri to the container settings");
        let auth_uri = self
            .auth_uri
            .take()
            .expect("to provide an auth uri to the container settings");
        let network_name = self
            .network_name
            .take()
            .expect("to provide a network name to the container settings");
        let runtime_start_channel = self
            .runtime_start_channel
            .take()
            .expect(" to provide a channel to be used for starting a runable deployment");
        let runnable_deployment = self
            .runnable_deployment
            .take()
            .expect("to provide a runnable deployment");

        ContainerSettings {
            prefix,
            provisioner_host,
            auth_uri,
            network_name,
            runnable_deployment,
            runtime_start_channel,
        }
    }
}

#[derive(Clone)]
pub struct ContainerSettings {
    pub prefix: String,
    pub provisioner_host: String,
    pub auth_uri: String,
    pub network_name: String,
    pub runnable_deployment: RunnableDeployment,
    pub runtime_start_channel: Sender<RunnableDeployment>,
}

impl ContainerSettings {
    pub fn builder() -> ContainerSettingsBuilder {
        ContainerSettingsBuilder::new()
    }
}

#[derive(Clone)]
pub struct ServiceDockerContext {
    docker: Docker,
    settings: Option<ContainerSettings>,
    runtime_manager: RuntimeManager,
}

impl ServiceDockerContext {
    pub fn new(docker: Docker, runtime_manager: RuntimeManager) -> Self {
        Self {
            docker,
            settings: None,
            runtime_manager,
        }
    }

    pub fn new_with_container_settings(
        docker: Docker,
        cs: ContainerSettings,
        runtime_manager: RuntimeManager,
    ) -> Self {
        Self {
            docker,
            settings: Some(cs),
            runtime_manager,
        }
    }
}

impl DockerContext for ServiceDockerContext {
    fn docker(&self) -> &Docker {
        &self.docker
    }

    fn container_settings(&self) -> Option<&ContainerSettings> {
        self.settings.as_ref()
    }

    fn runtime_manager(&self) -> RuntimeManager {
        self.runtime_manager.clone()
    }
}

pub trait DockerContext: Send + Sync {
    fn docker(&self) -> &Docker;

    fn container_settings(&self) -> Option<&ContainerSettings>;

    fn runtime_manager(&self) -> RuntimeManager;
}

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

pub trait ContainerInspectResponseExt {
    fn container(&self) -> &ContainerInspectResponse;

    fn service_id(&self) -> Result<Ulid, ServiceErrored> {
        let container = self.container();

        Ulid::from_string(safe_unwrap!(container
            .config
            .labels
            .get("shuttle.service_id")))
        .map_err(|err| ServiceErrored::internal(err.to_string()))
    }

    fn deployment_id(&self) -> Result<Ulid, ServiceErrored> {
        let container = self.container();

        Ulid::from_string(safe_unwrap!(container
            .config
            .labels
            .get("shuttle.deployment_id")))
        .map_err(|err| ServiceErrored::internal(err.to_string()))
    }

    fn service_name(&self) -> Result<String, ServiceErrored> {
        let container = self.container();
        if let Some(config) = &container.config {
            if let Some(labels) = &config.labels {
                if let Some(service_name) = labels.get("shuttle.service_name") {
                    return Ok(service_name.clone());
                }
            }
        }

        Err(ServiceErrored::internal(
            "missing service name label on the container inspect info",
        ))
    }

    fn image_name(&self) -> Result<String, ServiceErrored> {
        let container = self.container();
        if let Some(config) = &container.config {
            if let Some(image_name) = &config.image {
                return Ok(image_name.clone());
            }
        }

        Err(ServiceErrored::internal(
            "missing image name from the container inspect info",
        ))
    }

    fn idle_minutes(&self) -> u64 {
        let container = self.container();

        if let Some(config) = &container.config {
            if let Some(labels) = &config.labels {
                if let Some(idle_minutes) = labels.get("shuttle.idle_minutes") {
                    return idle_minutes.parse::<u64>().unwrap_or(IDLE_MINUTES);
                }
            }
        }

        IDLE_MINUTES
    }

    fn is_next(&self) -> bool {
        let container = self.container();

        if let Some(config) = &container.config {
            if let Some(labels) = &config.labels {
                if let Some(idle_minutes) = labels.get("shuttle.is_next") {
                    return idle_minutes.parse::<bool>().unwrap_or(false);
                }
            }
        }

        false
    }

    fn find_arg_and_then<'s, F, O>(&'s self, find: &str, and_then: F) -> Result<O, ServiceErrored>
    where
        F: FnOnce(&'s str) -> O,
        O: 's,
    {
        let mut args = self.args()?.iter();
        let out = if args.any(|arg| arg.as_str() == find) {
            args.next().map(|s| and_then(s.as_str()))
        } else {
            None
        };
        out.ok_or_else(|| ServiceErrored::internal(format!("no such argument: {find}")))
    }

    fn args(&self) -> Result<&Vec<String>, ServiceErrored> {
        let container = self.container();
        Ok(safe_unwrap!(container.args))
    }
}

impl ContainerInspectResponseExt for ContainerInspectResponse {
    fn container(&self) -> &ContainerInspectResponse {
        self
    }
}
