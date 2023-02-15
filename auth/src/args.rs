use std::{net::SocketAddr, path::PathBuf};

use clap::Parser;

#[derive(Parser, Debug)]
pub struct Args {
    /// Where to store auth state (such as users)
    #[arg(long, default_value = "./")]
    pub state: PathBuf,

    /// Address to bind to
    #[arg(long, default_value = "127.0.0.1:8000")]
    pub address: SocketAddr,
}
