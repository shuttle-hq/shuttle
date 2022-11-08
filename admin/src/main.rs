use clap::Parser;
use shuttle_admin::{
    args::{AcmeCommand, Args, Command},
    client::Client,
    config::get_api_key,
};
use std::fmt::Write;
use tracing::trace;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let args = Args::parse();

    trace!(?args, "starting with args");

    let api_key = get_api_key();
    let client = Client::new(args.api_url.clone(), api_key);

    let res = match args.command {
        Command::Revive => client.revive().await.expect("revive to succeed"),
        Command::Acme(AcmeCommand::CreateAccount { email }) => {
            let account = client
                .acme_account_create(&email)
                .await
                .expect("to create ACME account");

            let mut res = String::new();
            writeln!(res, "Details of ACME account are as follow. Keep this safe as it will be needed to create certificates in the future").unwrap();
            writeln!(res, "{}", serde_json::to_string_pretty(&account).unwrap()).unwrap();

            res
        }
    };

    println!("{res}");
}
