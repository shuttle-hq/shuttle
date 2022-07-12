use std::net::IpAddr;
use std::path::PathBuf;
use std::str::FromStr;

use clap::Parser;
use fqdn::FQDN;
use shuttle_common::Port;

#[derive(Parser)]
#[clap(name = "shuttle")]
pub struct Args {
    #[clap(long, help = "Override the default root path for shuttle")]
    pub(crate) path: Option<PathBuf>,
    #[clap(
        long,
        help = "Override the default port for the proxy",
        default_value = "8000"
    )]
    pub(crate) proxy_port: Port,
    #[clap(
        long,
        help = "Override the default port for the api",
        default_value = "8001"
    )]
    pub(crate) api_port: Port,
    #[clap(
        long,
        help = "Override the default bind address",
        default_value = "127.0.0.1"
    )]
    pub(crate) bind_addr: IpAddr,
    #[clap(long, help = "Fully qualified domain name deployed services are reachable at", parse(try_from_str = parse_fqdn))]
    pub(crate) proxy_fqdn: FQDN,
    #[clap(long, help = "Address to connect to the provisioning service")]
    pub(crate) provisioner_address: String,
    #[clap(
        long,
        help = "Port provisioner is reachable at",
        default_value = "5001"
    )]
    pub(crate) provisioner_port: Port,
}

fn parse_fqdn(src: &str) -> Result<FQDN, String> {
    FQDN::from_str(src).map_err(|e| format!("{e:?}"))
}
