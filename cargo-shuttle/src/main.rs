
use anyhow::Result;
use cargo_shuttle::{Args, Shuttle};
use structopt::StructOpt;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    Shuttle::new().run(Args::from_args()).await
}
