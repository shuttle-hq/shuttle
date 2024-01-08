use std::net::SocketAddr;

use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
pub struct Args {
    /// Where to store auth state (such as users)
    #[arg(long)]
    pub db_connection_uri: String,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    Start(StartArgs),
    InitAdmin(InitArgs),
    InitDeployer(InitArgs),
}

#[derive(clap::Args, Debug, Clone)]
pub struct StartArgs {
    /// Address to bind to
    #[arg(long, default_value = "127.0.0.1:8000")]
    pub address: SocketAddr,

    /// Stripe client secret key
    #[arg(long, default_value = "")]
    pub stripe_secret_key: String,

    /// Auth JWT signing private key, as a base64 encoding of
    /// a PEM encoded PKCS#8 v1 formatted unencrypted private key.
    #[arg(long, default_value = "")]
    pub jwt_signing_private_key: String,
}

#[derive(clap::Args, Debug, Clone)]
pub struct InitArgs {
    /// Name of initial account to create
    #[arg(long)]
    pub name: String,
    /// Key to assign to initial account
    #[arg(long)]
    pub key: Option<String>,
}
