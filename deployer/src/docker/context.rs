use bollard::{Docker, API_DEFAULT_VERSION};
use fqdn::FQDN;
use http::Uri;

use super::container::ContainerSettings;

#[derive(Debug, Clone)]
pub struct ContextArgs {
    /// Prefix to add to the name of all docker resources managed by
    /// this service
    pub prefix: String,
    /// The address at which an active runtime container will find
    /// the provisioner service
    pub provisioner_host: String,
    /// Address to reach the authentication service at
    pub auth_uri: Uri,
    /// The Docker Network name in which to deploy user runtimes
    pub network_name: String,
    /// FQDN where the proxy can be reached at
    pub proxy_fqdn: FQDN,
    /// The path to the docker daemon socket
    pub docker_host: String,
}

#[derive(Clone)]
pub struct ContextProvider {
    docker: Docker,
    settings: ContainerSettings,
}

impl ContextProvider {
    pub fn new(args: ContextArgs, settings: ContainerSettings) -> Self {
        let docker = Docker::connect_with_unix(&args.docker_host, 60, API_DEFAULT_VERSION)
            .expect("to get a docker client. Do you have docker installed?");
        Self { docker, settings }
    }

    pub fn docker(&self) -> &Docker {
        &self.docker
    }

    pub fn container_settings(&self) -> &ContainerSettings {
        &self.settings
    }
}
