use std::net::SocketAddr;

use clap::{Parser, Subcommand};
use http::Uri;
use shuttle_common::models::user::UserId;

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
    Sync(SyncArgs),
    /// Copy and overwrite a permit env's policies to another env.
    /// Requires a project level API key.
    CopyPermitEnv(CopyPermitEnvArgs),
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

    #[command(flatten)]
    pub permit: PermitArgs,
}

#[derive(clap::Args, Debug, Clone)]
pub struct InitArgs {
    /// User id of account to create
    #[arg(long)]
    pub user_id: UserId,
    /// Key to assign to initial account
    #[arg(long)]
    pub key: Option<String>,
}

#[derive(clap::Args, Debug, Clone)]
pub struct SyncArgs {
    #[command(flatten)]
    pub permit: PermitArgs,
}

#[derive(clap::Args, Debug, Clone)]
pub struct CopyPermitEnvArgs {
    /// environment to copy to
    pub target: String,
    #[command(flatten)]
    pub permit: PermitArgs,
}

#[derive(clap::Args, Debug, Clone)]
pub struct PermitArgs {
    /// Address to reach the permit.io API at
    #[arg(long, default_value = "https://api.eu-central-1.permit.io")]
    pub permit_api_uri: Uri,
    /// Address to reach the permit.io PDP at
    #[arg(long, default_value = "http://permit-pdp:7000")]
    pub permit_pdp_uri: Uri,
    /// Permit environment to use
    #[arg(long, default_value = "local")]
    pub permit_env: String,
    /// Permit API key
    #[arg(long, default_value = "permit_")]
    pub permit_api_key: String,
}
