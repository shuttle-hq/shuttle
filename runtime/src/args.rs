use clap::Parser;
use tonic::transport::Endpoint;

#[derive(Parser, Debug)]
pub struct Args {
    /// Address to reach provisioner at
    #[clap(long, default_value = "localhost:5000")]
    pub provisioner_address: Endpoint,

    /// Is this runtime for a legacy service
    #[clap(long, conflicts_with("axum"))]
    pub legacy: bool,

    /// Is this runtime for an axum-wasm service
    #[clap(long, conflicts_with("legacy"))]
    pub axum: bool,
}
