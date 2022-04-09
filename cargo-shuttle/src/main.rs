mod args;
mod client;
mod config;

use anyhow::Result;
use structopt::StructOpt;

use cargo_shuttle::{Args, Shuttle};

#[tokio::main]
async fn main() -> Result<()> {
    Shuttle::new().run(Args::from_args()).await
}
