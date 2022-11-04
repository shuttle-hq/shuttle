use clap::Parser;
use shuttle_admin::args::Args;

fn main() {
    let args = Args::parse();

    println!("{args:?}");
}
