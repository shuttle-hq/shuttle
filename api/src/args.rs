use std::net::IpAddr;
use std::path::PathBuf;
use std::str::FromStr;

use clap::Parser;
use fqdn::FQDN;
use shuttle_common::Port;

#[derive(Parser)]
#[clap(name = "shuttle")]
pub struct Args {
    /// Override the default root path for shuttle
    #[clap(long)]
    pub(crate) path: Option<PathBuf>,
    /// Override the default port for the proxy
    #[clap(long, default_value = "8000")]
    pub(crate) proxy_port: Port,
    /// Override the default port for the api
    #[clap(long, default_value = "8001")]
    pub(crate) api_port: Port,
    /// Override the default bind address
    #[clap(long, default_value = "127.0.0.1")]
    pub(crate) bind_addr: IpAddr,
    /// Fully qualified domain name deployed services are reachable at
    #[clap(long, parse(try_from_str = parse_fqdn))]
    pub(crate) proxy_fqdn: FQDN,
    /// Address to connect to the provisioning service
    #[clap(long)]
    pub(crate) provisioner_address: String,
    /// Port provisioner is reachable at
    #[clap(long, default_value = "5001")]
    pub(crate) provisioner_port: Port,
    #[structopt(
        long,
        about = "MSSV - Minimum supported Shuttle Version",
        default_value = shuttle_service::VERSION
    )]
    pub(crate) shuttle_version: semver::Version,
}

fn parse_fqdn(src: &str) -> Result<FQDN, String> {
    FQDN::from_str(src).map_err(|e| format!("{e:?}"))
}
