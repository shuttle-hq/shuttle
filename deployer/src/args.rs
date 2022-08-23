use clap::Parser;
use shuttle_common::Port;

/// Program to handle the deploys for a single project
/// Handling includes, building, testing, and running each service
#[derive(Parser)]
#[clap(author, version, about)]
pub struct Args {
    /// Address to connect to the provisioning service
    #[clap(long)]
    pub(crate) provisioner_address: String,

    /// Port provisioner is running on
    #[clap(long, default_value = "5000")]
    pub(crate) provisioner_port: Port,
}
