use clap::Parser;

#[derive(Parser, Debug)]
pub struct Args {
    /// Uri to the `.so` file to load
    #[arg(long, short)]
    pub file_path: String,
}
