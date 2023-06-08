use super::context::ContextArgs;

#[derive(Clone)]
pub struct ContainerSettings {
    pub prefix: String,
    pub provisioner_host: String,
    pub auth_uri: String,
    pub network_name: String,
    pub fqdn: String,
}

impl ContainerSettings {
    pub fn builder() -> ContainerSettingsBuilder {
        ContainerSettingsBuilder::new()
    }
}

pub struct ContainerSettingsBuilder {
    prefix: Option<String>,
    provisioner: Option<String>,
    auth_uri: Option<String>,
    network_name: Option<String>,
    fqdn: Option<String>,
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
            provisioner: None,
            auth_uri: None,
            network_name: None,
            fqdn: None,
        }
    }

    pub async fn from_args(self, args: &ContextArgs) -> ContainerSettings {
        let ContextArgs {
            prefix,
            network_name,
            provisioner_host,
            auth_uri,
            proxy_fqdn,
            ..
        } = args;
        self.prefix(prefix)
            .provisioner_host(provisioner_host)
            .auth_uri(auth_uri)
            .network_name(network_name)
            .fqdn(proxy_fqdn)
            .build()
            .await
    }

    pub fn prefix<S: ToString>(mut self, prefix: S) -> Self {
        self.prefix = Some(prefix.to_string());
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

    pub fn fqdn<S: ToString>(mut self, fqdn: S) -> Self {
        self.fqdn = Some(fqdn.to_string().trim_end_matches('.').to_string());
        self
    }

    pub async fn build(mut self) -> ContainerSettings {
        let prefix = self.prefix.take().unwrap();
        let provisioner_host = self.provisioner.take().unwrap();
        let auth_uri = self.auth_uri.take().unwrap();

        let network_name = self.network_name.take().unwrap();
        let fqdn = self.fqdn.take().unwrap();

        ContainerSettings {
            prefix,
            provisioner_host,
            auth_uri,
            network_name,
            fqdn,
        }
    }
}
