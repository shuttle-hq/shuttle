use std::{net::SocketAddr, path::PathBuf};

use clap::Parser;
use tonic::transport::{Endpoint, Uri};

#[derive(Parser, Debug)]
pub struct Args {
    /// Address to bind to
    #[arg(long, default_value = "127.0.0.1:8000")]
    pub address: SocketAddr,

    /// Where to store resources state
    #[arg(long, default_value = "./")]
    pub state: PathBuf,

    /// Address to reach the authentication service at
    #[arg(long, default_value = "http://127.0.0.1:8008")]
    pub auth_uri: Uri,

    /// Address to connect to the provisioning service
    #[clap(long, default_value = "http://provisioner:5000")]
    pub provisioner_uri: Endpoint,

    /// Uri to folder to store all artifacts
    #[clap(long, default_value = "/tmp")]
    pub artifacts_path: PathBuf,

    /// Address to reach gateway's control plane at
    #[clap(long, default_value = "http://gateway:8001")]
    pub gateway_uri: Uri,

    /// Used to prefix names for all docker resources
    #[clap(long, default_value = "shuttle_dev")]
    pub prefix: String,

    /// The overlay network name used for the user services
    #[clap(long, default_value = "shared_user_net")]
    pub network_name: String,
}
