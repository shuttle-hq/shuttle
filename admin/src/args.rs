use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
pub struct Args {
    /// run this command against the api at the supplied url
    #[arg(long, default_value = "https://api.shuttle.rs", env = "SHUTTLE_API")]
    pub api_url: String,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Try to revive projects in the crashed state
    Revive,
}
