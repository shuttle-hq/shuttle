use async_trait::async_trait;
use bollard::{
    container::{Config, CreateContainerOptions},
    errors::Error as DockerError,
    service::ContainerInspectResponse,
};
use futures::TryFutureExt;
use serde::{Deserialize, Serialize};
use shuttle_common::models::project::idle_minutes;
use tracing::{debug, instrument};

use super::machine::State;
use crate::{
    deserialize_json,
    project::{
        docker::{ContainerInspectResponseExt, ContainerSettings, DockerContext},
        service::error::Error,
    },
};

use super::{b_attaching::ServiceAttaching, m_errored::ServiceErrored};

// TODO: We need to send down the runtime_manager from the deployer-alpha
// Add the fields that are present in Built to the `ServiceCreating` (they will be persisted, maybe not all of them should be passed)
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ServiceCreating {
    /// The service Ulid
    service_id: String,
    /// Image used to create the container from
    image: String,
    /// Configuration will be extracted from there if specified (will
    /// take precedence over other overrides)
    from: Option<ContainerInspectResponse>,
    // Use default for backward compatibility. Can be removed when all projects in the DB have this property set
    #[serde(default)]
    pub recreate_count: usize,
    /// Label set on container as to how many minutes to wait before a project is considered idle
    #[serde(default = "idle_minutes")]
    idle_minutes: u64,
}

impl ServiceCreating {
    pub fn new(service_id: String, image_name: String, idle_minutes: u64) -> Self {
        Self {
            service_id,
            image: image_name,
            from: None,
            recreate_count: 0,
            idle_minutes,
        }
    }

    pub fn from_container(
        container: ContainerInspectResponse,
        recreate_count: usize,
    ) -> Result<Self, ServiceErrored> {
        let service_id = container.service_id()?;
        let idle_minutes = container.idle_minutes();

        Ok(Self {
            service_id: service_id.to_string(),
            image: container.image.clone().ok_or(ServiceErrored::internal(
                "container inspect response misses the image name",
            ))?,
            from: Some(container),
            recreate_count,
            idle_minutes,
        })
    }

    pub fn from(mut self, from: ContainerInspectResponse) -> Self {
        self.from = Some(from);
        self
    }

    pub fn service_id(&self) -> &String {
        &self.service_id
    }

    fn container_name<C: DockerContext>(&self, ctx: &C) -> String {
        let prefix = &ctx.container_settings().prefix;

        let Self { service_id, .. } = &self;

        format!("{prefix}{service_id}_run")
    }

    fn generate_container_config<C: DockerContext>(
        &self,
        ctx: &C,
    ) -> Result<(CreateContainerOptions<String>, Config<String>), Error> {
        let ContainerSettings {
            prefix,
            is_next,
            provisioner_host,
            auth_uri,
            ..
        } = ctx.container_settings();

        let Self {
            service_id,
            image,
            idle_minutes,
            ..
        } = &self;

        let create_container_options = CreateContainerOptions {
            name: self.container_name(ctx),
            platform: None,
        };

        // TODO: pull the image from the registry, inspect it and retrieve the image config CMD,
        // use that to get the executable with the shuttle-runtime, because otherwise, when we're
        // creating the create container config we're overwriting the executable path and it can
        // not be found afterward.
        let mut cmd = vec!["--port", "8001"];
        // Currently, shuttle-next doesn't support a significant amount of Shuttle resources, so
        // we're completting the args here only for the alpha runtime.
        if !*is_next {
            cmd.extend([
                "--storage-manager-type",
                "artifacts",
                "--storage-manager-path",
                "/opt/shuttle",
                "--provisioner-address",
                provisioner_host.as_str(),
                "--auth-uri",
                auth_uri.as_str(),
            ]);
        };

        let container_config = self
            .from
            .as_ref()
            .and_then(|container| container.config.clone())
            .unwrap_or_else(|| {
                deserialize_json!({
                    "Image": image,
                    "Hostname": format!("{prefix}{service_id}"), // TODO: add volumes migration APIs
                    "Labels": {
                        "shuttle.service_id": service_id,
                        "shuttle.idle_minutes": format!("{idle_minutes}"),
                    },
                    "Cmd": cmd[..],
                    "Env": [
                        "RUST_LOG=debug,shuttle=trace,h2=warn",
                    ],
                    "ExposedPorts": {
                        "8001/tcp": {}
                    }
                })
            });

        let mut config = Config::<String>::from(container_config);

        config.host_config = deserialize_json!({
            "Mounts": [{
                "Target": "/opt/shuttle",
                "Source": format!("{prefix}{service_id}_vol"),
                "Type": "volume"
            }],
            // https://docs.docker.com/config/containers/resource_constraints/#memory
            "Memory": 6442450000i64, // 6 GiB hard limit
            "MemoryReservation": 4295000000i64, // 4 GiB soft limit, applied if host is low on memory
            // https://docs.docker.com/config/containers/resource_constraints/#cpu
            "CpuPeriod": 100000i64,
            "CpuQuota": 400000i64
        });

        debug!(
            r"generated a container configuration:
CreateContainerOpts: {create_container_options:#?}
Config: {config:#?}
"
        );

        Ok((create_container_options, config))
    }
}

#[async_trait]
impl<Ctx> State<Ctx> for ServiceCreating
where
    Ctx: DockerContext,
{
    type Next = ServiceAttaching;
    type Error = ServiceErrored;

    #[instrument(skip_all)]
    async fn next(self, ctx: &Ctx) -> Result<Self::Next, Self::Error> {
        let container_name = self.container_name(ctx);
        let Self { recreate_count, .. } = self;

        let container = ctx
            .docker()
            // If container already exists, use that
            .inspect_container(&container_name.clone(), None)
            // Otherwise create it
            .or_else(|err| async move {
                if matches!(err, DockerError::DockerResponseServerError { status_code, .. } if status_code == 404) {
                    let (opts, config) = self.generate_container_config(ctx).map_err(|err| ServiceErrored::internal(err.to_string()))?;
                    ctx.docker()
                        .create_container(Some(opts), config)
                        .and_then(|_| ctx.docker().inspect_container(&container_name, None))
                        .await
                        .map_err(ServiceErrored::from)
                } else {
                    Err(ServiceErrored::from(err))
                }
            })
            .await?;
        Ok(ServiceAttaching {
            container,
            recreate_count,
        })
    }
}
