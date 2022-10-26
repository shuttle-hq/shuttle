use clap::Parser;
use tonic::transport::Endpoint;

#[derive(Parser, Debug)]
pub struct Args {
    /// Address to reach provisioner at
    #[clap(long, default_value = "localhost:5000")]
    pub provisioner_address: Endpoint,
}
