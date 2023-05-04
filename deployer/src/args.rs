use std::{net::SocketAddr, path::PathBuf};

use clap::Parser;
use fqdn::FQDN;
use hyper::Uri;
use shuttle_common::project::ProjectName;
use tonic::transport::Endpoint;

/// Program to handle the deploys for a single project
/// Handling includes, building, testing, and running each service
#[derive(Debug, Parser)]
#[clap(author, version, about)]
pub struct Args {
    /// Uri to the `.sqlite` file used to store state
    #[clap(long, default_value = "./deployer.sqlite")]
    pub state: String,

    /// Address to connect to the provisioning service
    #[clap(long, default_value = "http://provisioner:3000")]
    pub provisioner_address: Endpoint,

    /// FQDN where the proxy can be reached at
    #[clap(long)]
    pub proxy_fqdn: FQDN,

    /// Address to bind API to
    #[clap(long, default_value = "0.0.0.0:8001")]
    pub api_address: SocketAddr,

    /// Address to bind proxy to
    #[clap(long, default_value = "0.0.0.0:8000")]
    pub proxy_address: SocketAddr,

    /// Address to reach gateway's control plane at
    #[clap(long, default_value = "http://gateway:8001")]
    pub gateway_uri: Uri,

    /// Project being served by this deployer
    #[clap(long)]
    pub project: ProjectName,

    /// Secret that will be used to perform admin tasks on this deployer
    #[clap(long)]
    pub admin_secret: String,

    /// Address to reach the authentication service at
    #[clap(long, default_value = "http://127.0.0.1:8008")]
    pub auth_uri: Uri,

    /// Uri to folder to store all artifacts
    #[clap(long, default_value = "/tmp")]
    pub artifacts_path: PathBuf,

    /// Add an auth layer to deployer for local development
    #[arg(long)]
    pub local: bool,
}
