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
    #[arg(long, default_value = "http://127.0.0.1:8008")]
    pub auth_uri: Uri,
}
