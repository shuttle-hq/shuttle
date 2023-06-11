use bollard::{
    container::{Config, CreateContainerOptions},
    service::ContainerInspectResponse,
};
use portpicker::pick_unused_port;
use serde::{Deserialize, Serialize};
use shuttle_common::models::project::idle_minutes;

use crate::project::{
    docker::{ContainerSettings, DockerContext},
    state::error::Error,
};

use super::error::ProjectError;

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

// TODO: We need to send down the runtime_manager from the deployer-alpha
// Add the fields that are present in Built to the `ServiceCreating` (they will be persisted, maybe not all of them should be passed)
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ServiceCreating {
    /// The service name
    service_name: String,
    /// The service Ulid
    service_id: String,
    /// Override the default image (specified in the args to this gateway)
    image: Option<String>,
    /// Configuration will be extracted from there if specified (will
    /// take precedence over other overrides)
    from: Option<ContainerInspectResponse>,
    // Use default for backward compatibility. Can be removed when all projects in the DB have this property set
    #[serde(default)]
    recreate_count: usize,
    /// Label set on container as to how many minutes to wait before a project is considered idle
    #[serde(default = "idle_minutes")]
    idle_minutes: u64,
}

impl ServiceCreating {
    pub fn new(
        service_name: String,
        service_id: String,
        initial_key: String,
        idle_minutes: u64,
    ) -> Self {
        Self {
            service_name,
            service_id,
            image: None,
            from: None,
            recreate_count: 0,
            idle_minutes,
        }
    }

    pub fn from_container(
        container: ContainerInspectResponse,
        recreate_count: usize,
    ) -> Result<Self, ProjectError> {
        let service_name = container.service_name();
        let service_id = container.service_id()?;
        let idle_minutes = container.idle_minutes();
        let initial_key = container.initial_key()?;

        Ok(Self {
            service_name,
            service_id,
            image: None,
            from: Some(container),
            recreate_count,
            idle_minutes,
        })
    }

    pub fn from(mut self, from: ContainerInspectResponse) -> Self {
        self.from = Some(from);
        self
    }

    pub fn with_image(mut self, image: String) -> Self {
        self.image = Some(image);
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
    ) -> (CreateContainerOptions<String>, Config<String>) {
        let ContainerSettings {
            image: default_image,
            prefix,
            provisioner_host,
            auth_uri,
            fqdn: public,
            ..
        } = ctx.container_settings();

        let Self {
            initial_key,
            service_id,
            fqdn,
            image,
            idle_minutes,
            ..
        } = &self;

        let create_container_options = CreateContainerOptions {
            name: self.container_name(ctx),
            platform: None,
        };

        let port = match pick_unused_port() {
            Some(port) => port,
            None => {
                return Err(Error::RuntimePrepare(
                    "could not find a free port to deploy service on".to_string(),
                ))
            }
        };

        let container_config = self
            .from
            .as_ref()
            .and_then(|container| container.config.clone())
            .unwrap_or_else(|| {
                deserialize_json!({
                    "Image": image.as_ref().unwrap_or(default_image),
                    "Hostname": format!("{prefix}{service_id}"),
                    "Labels": {
                        "shuttle.prefix": prefix,
                        "shuttle.service_id": service_id,
                        "shuttle.idle_minutes": format!("{idle_minutes}"),
                    },
                    "Cmd": [
                        "--port",
                        port,
                        "--storage-manager-type",
                        storage_manager_type,
                        "--storage-manager-path",
                        storage_manager_path
                    ],
                    "Env": [
                        "RUST_LOG=debug,shuttle=trace,h2=warn",
                    ]
                })
            });

        let mut config = Config::<String>::from(container_config);

        config.host_config = deserialize_json!({
            "Mounts": [{
                "Target": "/opt/shuttle",
                "Source": format!("{prefix}{project_name}_vol"),
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

        (create_container_options, config)
    }
}

#[async_trait]
impl<Ctx> State<Ctx> for ProjectCreating
where
    Ctx: DockerContext,
{
    type Next = ProjectAttaching;
    type Error = ProjectError;

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
        Ok(ProjectAttaching {
            container,
            recreate_count,
        })
    }
}
