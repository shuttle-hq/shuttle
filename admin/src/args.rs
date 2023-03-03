use std::{fs, io, path::PathBuf};

use clap::{Error, Parser, Subcommand};
use shuttle_common::project::ProjectName;

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

    /// Destroy all the current running projects
    Destroy,

    /// Manage custom domains
    #[command(subcommand)]
    Acme(AcmeCommand),

    /// Manage project names
    ProjectNames,

    /// Viewing and managing stats
    #[command(subcommand)]
    Stats(StatsCommand),
}

#[derive(Subcommand, Debug)]
pub enum AcmeCommand {
    /// Create a new ACME account. Should only be needed once
    CreateAccount {
        /// Email for managing all certificates
        #[arg(long)]
        email: String,

        /// Acme server to create account on. Gateway will default to LetsEncrypt
        #[arg(long)]
        acme_server: Option<String>,
    },

    /// Request a certificate for a FQDN
    Request {
        /// Fqdn to request certificate for
        #[arg(long)]
        fqdn: String,

        /// Project to request certificate for
        #[arg(long)]
        project: ProjectName,

        /// Path to acme credentials file
        /// This should have been created with `acme create-account`
        #[arg(long, value_parser = load_credentials)]
        credentials: serde_json::Value,
    },

    /// Renew the certificate for a FQDN
    RenewCustomDomain {
        /// Fqdn to renew the certificate for
        #[arg(long)]
        fqdn: String,

        /// Project to renew the certificate for
        #[arg(long)]
        project: ProjectName,

        /// Path to acme credentials file
        /// This should have been created with `acme create-account`
        #[arg(long, value_parser = load_credentials)]
        credentials: serde_json::Value,
    },

    /// Renew the certificate for the shuttle gateway
    RenewGateway {
        /// Path to acme credentials file
        /// This should have been created with `acme create-account`
        #[arg(long, value_parser = load_credentials)]
        credentials: serde_json::Value,
    },
}

#[derive(Subcommand, Debug)]
pub enum StatsCommand {
    /// View load stats
    Load {
        /// Clear the loads counter
        #[arg(long)]
        clear: bool,
    },
}

fn load_credentials(s: &str) -> Result<serde_json::Value, Error> {
    let credentials = fs::read_to_string(PathBuf::from(s))?;
    serde_json::from_str(&credentials).map_err(|err| Error::from(io::Error::from(err)))
}
