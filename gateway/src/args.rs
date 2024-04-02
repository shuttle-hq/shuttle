use std::{net::SocketAddr, path::PathBuf};

use clap::{Parser, Subcommand, ValueEnum};
use fqdn::FQDN;
use http::Uri;

#[derive(Parser, Debug)]
pub struct Args {
    /// Where to store gateway state (sqlite and certs)
    #[arg(long, default_value = ".")]
    pub state: PathBuf,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum UseTls {
    Disable,
    Enable,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    Start(StartArgs),
    Sync(SyncArgs),
}

#[derive(clap::Args, Debug, Clone)]
pub struct StartArgs {
    /// Address to bind the control plane to
    #[arg(long, default_value = "127.0.0.1:8001")]
    pub control: SocketAddr,
    /// Address to bind the bouncer service to
    #[arg(long, default_value = "127.0.0.1:7999")]
    pub bouncer: SocketAddr,
    /// Address to bind the user proxy to
    #[arg(long, default_value = "127.0.0.1:8000")]
    pub user: SocketAddr,
    /// Allows to disable the use of TLS in the user proxy service (DANGEROUS)
    #[arg(long, default_value = "enable")]
    pub use_tls: UseTls,
    /// The origin to allow CORS requests from
    #[arg(long, default_value = "https://console.shuttle.rs")]
    pub cors_origin: String,
    #[command(flatten)]
    pub context: ServiceArgs,
    #[command(flatten)]
    pub permit: PermitArgs,
}

#[derive(clap::Args, Debug, Clone)]
pub struct ServiceArgs {
    /// Default image to deploy user runtimes into
    #[arg(long, default_value = "public.ecr.aws/shuttle/deployer:latest")]
    pub image: String,
    /// Prefix to add to the name of all docker resources managed by
    /// this service
    #[arg(long, default_value = "shuttle_prod_")]
    pub prefix: String,
    /// The address at which an active runtime container will find
    /// the provisioner service
    #[arg(long, default_value = "provisioner")]
    pub provisioner_host: String,
    /// Address to reach the authentication service at
    #[arg(long, default_value = "http://127.0.0.1:8008")]
    pub auth_uri: Uri,
    /// Address to reach the resource recorder service at
    #[arg(long, default_value = "http://resource-recorder:8000")]
    pub resource_recorder_uri: Uri,
    /// The Docker Network name in which to deploy user runtimes
    #[arg(long, default_value = "shuttle_default")]
    pub network_name: String,
    /// FQDN where the proxy can be reached at
    #[arg(long, default_value = "shuttleapp.rs")]
    pub proxy_fqdn: FQDN,
    /// The path to the docker daemon socket
    #[arg(long, default_value = "/var/run/docker.sock")]
    pub docker_host: String,
    /// API key used by the gateway to authorize API keys to JWTs conversion
    #[arg(long)]
    pub admin_key: String,
    /// Api key for the user that has rights to start deploys
    #[arg(long, default_value = "gateway4deployes")]
    pub deploys_api_key: String,

    /// Maximum number of containers to start on this node before blocking cch projects
    #[arg(long, default_value = "900")]
    pub cch_container_limit: u32,
    /// Maximum number of containers to start on this node before blocking non-pro projects
    #[arg(long, default_value = "970")]
    pub soft_container_limit: u32,
    /// Maximum number of containers to start on this node before blocking any project
    #[arg(long, default_value = "990")]
    pub hard_container_limit: u32,

    /// Allow tests to set some extra /etc/hosts
    pub extra_hosts: Vec<String>,
}

#[derive(clap::Args, Debug, Clone)]
pub struct SyncArgs {
    #[command(flatten)]
    pub permit: PermitArgs,
}

#[derive(clap::Args, Debug, Clone)]
pub struct PermitArgs {
    /// Address to reach the permit.io API at
    #[arg(long, default_value = "https://api.eu-central-1.permit.io")]
    pub permit_api_uri: Uri,
    /// Address to reach the permit.io PDP at
    #[arg(long, default_value = "http://permit-pdp:7000")]
    pub permit_pdp_uri: Uri,
    /// Permit environment to use
    #[arg(long, default_value = "local")]
    pub permit_env: String,
    /// Permit API key
    #[arg(long, default_value = "permit_")]
    pub permit_api_key: String,
}
