use std::net::IpAddr;
use std::path::PathBuf;
use std::str::FromStr;

use fqdn::FQDN;
use shuttle_common::Port;
use structopt::StructOpt;

#[derive(StructOpt)]
#[structopt(name = "shuttle")]
pub struct Args {
    #[structopt(long, about = "Override the default root path for shuttle")]
    pub(crate) path: Option<PathBuf>,
    #[structopt(
        long,
        about = "Override the default port for the proxy",
        default_value = "8000"
    )]
    pub(crate) proxy_port: Port,
    #[structopt(
        long,
        about = "Override the default port for the api",
        default_value = "8001"
    )]
    pub(crate) api_port: Port,
    #[structopt(
        long,
        about = "Override the default bind address",
        default_value = "127.0.0.1"
    )]
    pub(crate) bind_addr: IpAddr,
    #[structopt(long, about = "Fully qualified domain name deployed services are reachable at", parse(try_from_str = parse_fqdn))]
    pub(crate) fqdn: FQDN,
}

fn parse_fqdn(src: &str) -> Result<FQDN, String> {
    FQDN::from_str(src).map_err(|e| format!("{e:?}"))
}
