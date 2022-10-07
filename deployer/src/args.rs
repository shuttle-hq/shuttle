use std::{net::SocketAddr, path::PathBuf};

use clap::Parser;
use fqdn::FQDN;
use shuttle_common::Port;

/// Program to handle the deploys for a single project
/// Handling includes, building, testing, and running each service
#[derive(Debug, Parser)]
#[clap(author, version, about)]
pub struct Args {
    /// Address to connect to the provisioning service
    #[clap(long)]
    pub provisioner_address: String,

    /// Port provisioner is running on
    #[clap(long, default_value = "5000")]
    pub provisioner_port: Port,

    /// FQDN where the proxy can be reached at
    #[clap(long)]
    pub proxy_fqdn: FQDN,

    /// Address to bind API to
    #[clap(long, default_value = "0.0.0.0:8001")]
    pub api_address: SocketAddr,

    /// Address to bind proxy to
    #[clap(long, default_value = "0.0.0.0:8000")]
    pub proxy_address: SocketAddr,

    /// Secret that will be used to perform admin tasks on this deployer
    #[clap(long)]
    pub admin_secret: String,

    /// Uri to folder to store all artifacts
    #[clap(long, default_value = "/tmp")]
    pub artifacts_path: PathBuf,
}
