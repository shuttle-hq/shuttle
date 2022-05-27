use anyhow::Result;
use cargo_shuttle::{Args, Shuttle};
use structopt::StructOpt;

#[tokio::main]
async fn main() -> Result<()> {
    Shuttle::new().run(Args::from_args()).await
}
