use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;

use service::Service;

struct Deployment {
    /// A user's particular implementation of the [`Service`] trait.
    service: Box<dyn Service>,
    /// This [`libloading::Library`] instance must be kept alive in order to use
    /// the `service` field without causing a segmentation fault.
    lib: libloading::Library,
}

pub(crate) trait DeploySystem: Send + Sync {
    fn deploy(&mut self, project_name: String, so_path: &Path) -> Result<()>;
}

const ENTRYPOINT_NAME: &'static [u8] = b"_create_service\0";

type CreateService = unsafe extern fn() -> *mut dyn Service;

#[derive(Default)]
pub(crate) struct ServiceDeploySystem {
    deployments: HashMap<String, Deployment>,
}

impl DeploySystem for ServiceDeploySystem {
    /// Dynamically load from a `.so` file a value of a type implementing the
    /// [`Service`] trait. Relies on the `.so` library having an ``extern "C"`
    /// function called [`ENTRYPOINT_NAME`], likely automatically generated
    /// using the [`service::declare_service`] macro.
    fn deploy(&mut self, project_name: String, so_path: &Path) -> Result<()> {
        let (service, lib) = unsafe {
            let lib = libloading::Library::new(so_path)?;

            let entrypoint: libloading::Symbol<CreateService> = lib.get(ENTRYPOINT_NAME)?;
            let raw = entrypoint();

            (Box::from_raw(raw), lib)
        };

        let deployment = Deployment {
            service,
            lib,
        };

        self.deployments.insert(project_name, deployment);

        Ok(())
    }
}
