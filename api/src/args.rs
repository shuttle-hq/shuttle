use std::path::PathBuf;
use structopt::StructOpt;

#[derive(StructOpt)]
#[structopt(
name = "unveil",
about = "synthetic data engine on the command line",
)]
pub struct Args {
    #[structopt(long, about = "Override the default root path for unveil")]
    pub(crate) path: Option<PathBuf>,
}