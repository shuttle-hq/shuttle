use std::net::SocketAddr;

use clap::Parser;

#[derive(Parser, Debug, Clone)]
pub struct Args {
    /// Address to bind the control plane to
    #[clap(long, default_value = "127.0.0.1:8001")]
    pub control: SocketAddr,
    /// Address to bind the user plane to
    #[clap(long, default_value = "127.0.0.1:8000")]
    pub user: SocketAddr,
    /// Default image to deploy user runtimes into
    #[clap(long, default_value = "public.ecr.aws/shuttle/backend:latest")]
    pub image: String,
    /// Prefix to add to the name of all docker resources managed by
    /// this service
    #[clap(long, default_value = "shuttle_prod_")]
    pub prefix: String,
    /// The address at which an active runtime container will find
    /// the provisioner service
    #[clap(long, default_value = "provisioner")]
    pub provisioner_host: String,
    /// The Docker Network name in which to deploy user runtimes
    #[clap(long, default_value = "shuttle_default")]
    pub network_name: String,
    /// Uri to the `.sqlite` file used to store state
    #[clap(long, default_value = "./gateway.sqlite")]
    pub state: String
}
