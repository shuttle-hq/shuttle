use clap::Parser;

#[derive(Parser, Debug)]
#[command(version)]
pub struct NextArgs {
    /// Port to start runtime on
    #[arg(long)]
    pub port: u16,
}
