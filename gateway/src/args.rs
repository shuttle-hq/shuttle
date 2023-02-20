use std::{net::SocketAddr, path::PathBuf};

use clap::{Parser, Subcommand, ValueEnum};
use fqdn::FQDN;

#[derive(Parser, Debug)]
pub struct Args {
    /// Where to store gateway state (such as sqlite state, and certs)
    #[arg(long, default_value = "./")]
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
    /// Address to reach the authentication service at
    #[arg(long, default_value = "127.0.0.1:8008")]
    pub auth_service: SocketAddr,
    /// Allows to disable the use of TLS in the user proxy service (DANGEROUS)
    #[arg(long, default_value = "enable")]
    pub use_tls: UseTls,
    #[command(flatten)]
    pub context: ContextArgs,
}

#[derive(clap::Args, Debug, Clone)]
pub struct ContextArgs {
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
    /// The Docker Network name in which to deploy user runtimes
    #[arg(long, default_value = "shuttle_default")]
    pub network_name: String,
    /// FQDN where the proxy can be reached at
    #[arg(long, default_value = "shuttleapp.rs")]
    pub proxy_fqdn: FQDN,
    /// The path to the docker daemon socket
    #[arg(long, default_value = "/var/run/docker.sock")]
    pub docker_host: String,
}
