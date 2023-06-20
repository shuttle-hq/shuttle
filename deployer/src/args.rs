use std::{net::SocketAddr, path::PathBuf};

use clap::Parser;
use tonic::transport::Uri;

#[derive(Parser, Debug)]
pub struct Args {
    /// Address to bind to
    #[arg(long, default_value = "127.0.0.1:8000")]
    pub address: SocketAddr,

    /// Where to store resources state
    #[arg(long, default_value = "./")]
    pub state: PathBuf,

    /// Address to reach the authentication service at
    #[arg(long, default_value = "http://auth:8008")]
    pub auth_uri: Uri,

    /// Address to connect to the provisioning service
    #[clap(long, default_value = "http://provisioner:5001")]
    pub provisioner_uri: Uri,

    /// Used to prefix names for all docker resources
    #[clap(long, default_value = "shuttle_dev")]
    pub prefix: String,

    /// The overlay network name used for the user services
    #[clap(long, default_value = "shuttle_dev_user-net")]
    pub users_network_name: String,

    /// The path to the docker daemon socket
    #[arg(long, default_value = "/var/run/docker.sock")]
    pub docker_host: PathBuf,
}
