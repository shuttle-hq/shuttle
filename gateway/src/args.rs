use std::net::SocketAddr;

use clap::{Parser, Subcommand};
use fqdn::FQDN;

use crate::auth::Key;

#[derive(Parser, Debug)]
pub struct Args {
    /// Uri to the `.sqlite` file used to store state
    #[arg(long, default_value = "./gateway.sqlite")]
    pub state: String,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    Start(StartArgs),
    Init(InitArgs),
}

#[derive(clap::Args, Debug, Clone)]
pub struct StartArgs {
    /// Address to bind the control plane to
    #[arg(long, default_value = "127.0.0.1:8001")]
    pub control: SocketAddr,
    /// Address to bind the user plane to
    #[arg(long, default_value = "127.0.0.1:8000")]
    pub user: SocketAddr,
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
    #[arg(long)]
    pub proxy_fqdn: FQDN,
}

#[derive(clap::Args, Debug, Clone)]
pub struct InitArgs {
    /// Name of initial account to create
    #[arg(long)]
    pub name: String,
    /// Key to assign to initial account
    #[arg(long)]
    pub key: Option<Key>,
}
