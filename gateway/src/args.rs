use std::{net::SocketAddr, path::PathBuf};

use clap::{Parser, ValueEnum};
use fqdn::FQDN;
use http::Uri;

#[derive(Clone, Parser)]
pub struct Args {
    /// Address to bind the control plane to
    #[arg(long, default_value = "127.0.0.1:8001")]
    pub control: SocketAddr,
    /// Address to bind the bouncer service to
    #[arg(long, default_value = "127.0.0.1:7999")]
    pub bouncer: SocketAddr,
    /// Address to bind the user proxy to
    #[arg(long, default_value = "127.0.0.1:8000")]
    pub user: SocketAddr,
    /// Allows to disable the use of TLS in the user proxy service (DANGEROUS)
    #[arg(long, default_value = "enable")]
    pub use_tls: UseTls,
    /// Address to reach the authentication service at
    #[arg(long, default_value = "http://127.0.0.1:8008")]
    pub auth_uri: Uri,
    /// FQDN where the proxy can be reached at
    #[arg(long, default_value = "shuttleapp.rs")]
    pub proxy_fqdn: FQDN,
    /// Where to store gateway state (such as sqlite state, and certs)
    #[arg(long, default_value = "./")]
    pub state: PathBuf,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum UseTls {
    Disable,
    Enable,
}
