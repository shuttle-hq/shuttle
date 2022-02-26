use std::path::PathBuf;
use structopt::StructOpt;
use lib::Port;

#[derive(StructOpt)]
#[structopt(
name = "unveil",
about = "synthetic data engine on the command line",
)]
pub struct Args {
    #[structopt(long, about = "Override the default root path for unveil")]
    pub(crate) path: Option<PathBuf>,
    #[structopt(long, about = "Override the default port for the API", default_value = "8001")]
    pub(crate) api_port: Port,
    #[structopt(long, about = "Override the default port for the proxy", default_value = "8000")]
    pub(crate) proxy_port: Port,
}