use std::net::SocketAddr;

use clap::Parser;

#[derive(Parser, Debug)]
pub struct Args {
    /// Address to bind to
    #[arg(long, default_value = "127.0.0.1:8000")]
    pub address: SocketAddr,
}
