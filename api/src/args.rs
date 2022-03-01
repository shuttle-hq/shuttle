use lib::Port;
use std::net::IpAddr;
use std::path::PathBuf;
use structopt::StructOpt;

#[derive(StructOpt)]
#[structopt(name = "unveil")]
pub struct Args {
    #[structopt(long, about = "Override the default root path for unveil")]
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
}
