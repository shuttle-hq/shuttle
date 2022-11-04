use clap::Parser;
use shuttle_admin::{
    args::{Args, Command},
    client::Client,
    config::get_api_key,
};

#[tokio::main]
async fn main() {
    let args = Args::parse();
    let api_key = get_api_key();
    let client = Client::new(args.api_url.clone(), api_key);

    let res = match args.command {
        Command::Revive => client.revive().await.expect("revive to succeed"),
    };

    println!("{res}");
}
