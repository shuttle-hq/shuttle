use std::path::PathBuf;

use clap::Parser;

#[derive(Parser, Debug)]
pub struct Args {
    /// Where to store auth state (such as users)
    #[arg(long, default_value = "./")]
    pub state: PathBuf,
}
