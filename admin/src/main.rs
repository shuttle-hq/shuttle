use clap::Parser;
use shuttle_admin::{args::Args, config::get_api_key};

fn main() {
    let args = Args::parse();
    let api_key = get_api_key();

    println!("{args:?}");
    println!("{api_key}");
}
