use std::{net::SocketAddr, path::PathBuf};

use clap::Parser;
use hyper::Uri;

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
    pub provisioner_address: Uri,

    /// Address to connect to the logger service
    #[clap(long, default_value = "http://logger:8000")]
    pub logger_uri: Uri,

    /// Address to bind API to
    #[clap(long, default_value = "0.0.0.0:8001")]
    pub api_address: SocketAddr,

    /// Address to reach gateway's control plane at
    #[clap(long, default_value = "http://gateway:8001")]
    pub gateway_uri: Uri,

    /// Address to reach resource-recorder service at
    #[clap(long, default_value = "http://resource-recorder:8000")]
    pub resource_recorder: Uri,

    /// Project being served by this deployer
    #[clap(long)]
    pub project: String,

    /// Project id of the project of this deployer
    #[clap(long)]
    pub project_id: String,

    /// Secret that will be used to perform admin tasks on this deployer
    #[clap(long)]
    pub admin_secret: String,

    /// Address to reach the authentication service at
    #[clap(long, default_value = "http://auth:8000")]
    pub auth_uri: Uri,

    /// Uri to folder to store all artifacts
    #[clap(long, default_value = "/tmp")]
    pub artifacts_path: PathBuf,

    /// Add an auth layer to deployer for local development
    #[arg(long)]
    pub local: bool,
}
