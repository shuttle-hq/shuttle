use std::net::SocketAddr;

use clap::Parser;
use http::Uri;

/// Service to deploy projects on the shuttle infrastructure
#[derive(Debug, Parser)]
#[clap(author, version, about)]
pub struct Args {
    /// Address to reach the authentication service at
    #[clap(long, default_value = "http://127.0.0.1:8008")]
    pub auth_uri: Uri,

    /// Address to bind API to
    #[clap(long, default_value = "0.0.0.0:8001")]
    pub api_address: SocketAddr,

    /// Flag used to prepare the deployer for a local run
    #[clap(long)]
    pub local: bool,
}
