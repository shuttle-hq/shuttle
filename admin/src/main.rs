use clap::Parser;
use shuttle_admin::{
    args::{AcmeCommand, Args, Command},
    client::Client,
    config::get_api_key,
};
use std::{fmt::Write, fs};
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
        Command::Acme(AcmeCommand::RequestCertificate {
            fqdn,
            project,
            credentials,
        }) => {
            let credentials = fs::read_to_string(credentials).expect("to read credentials file");
            let credentials =
                serde_json::from_str(&credentials).expect("to parse content of credentials file");

            client
                .acme_request_certificate(&fqdn, &project, &credentials)
                .await
                .expect("to get a certificate challenge response")
        }
    };

    println!("{res}");
}
