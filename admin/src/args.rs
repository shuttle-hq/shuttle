use std::path::PathBuf;

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

    #[command(subcommand)]
    Acme(AcmeCommand),
}

#[derive(Subcommand, Debug)]
pub enum AcmeCommand {
    /// Create a new ACME account. Should only be needed once
    CreateAccount {
        /// Email for managing all certificates
        #[arg(long)]
        email: String,
    },

    /// Request a certificate for a FQDN
    RequestCertificate {
        /// Fqdn to request certificate for
        #[arg(long)]
        fqdn: String,

        /// Path to acme credentials file
        /// This should have been created with `acme create-account`
        #[arg(long)]
        credentials: PathBuf,
    },
}
