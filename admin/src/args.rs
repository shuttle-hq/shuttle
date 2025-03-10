use clap::{Parser, Subcommand};
use shuttle_common::{constants::API_URL_DEFAULT_BETA, models::project::ComputeTier};

#[derive(Parser, Debug)]
pub struct Args {
    /// run this command against the api at the supplied url
    #[arg(long, default_value = API_URL_DEFAULT_BETA, env = "SHUTTLE_API")]
    pub api_url: String,

    #[command(subcommand)]
    pub command: Command,

    /// Request timeout for the API client in seconds.
    #[arg(long, default_value_t = 120)]
    pub client_timeout: u64,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    ChangeProjectOwner {
        project_name: String,
        new_user_id: String,
    },

    UpdateCompute {
        /// Project to update
        #[arg(long, visible_alias = "id")]
        project_id: String,
        /// Compute tier to set.
        #[arg(long, visible_alias = "tier")]
        compute_tier: ComputeTier,
    },

    /// Renew all old custom domain certificates
    RenewCerts,

    AddFeatureFlag {
        entity: String,
        flag: String,
    },
    RemoveFeatureFlag {
        entity: String,
        flag: String,
    },

    /// Garbage collect free tier projects
    Gc {
        /// days since last deployment to filter by
        days: u32,
        /// loop and stop the returned projects instead of printing them
        #[arg(long)]
        stop_deployments: bool,
        /// limit how many projects to stop
        #[arg(long, default_value_t = 100)]
        limit: u32,
    },
    /// Garbage collect shuttlings projects
    GcShuttlings {
        /// minutes since last deployment to filter by
        minutes: u32,
        /// loop and stop the returned projects instead of printing them
        #[arg(long)]
        stop_deployments: bool,
        /// limit how many projects to stop
        #[arg(long, default_value_t = 100)]
        limit: u32,
    },

    /// Run a command with SHUTTLE_API_KEY set to a different user's key
    SimulateUser {
        /// user to simulate
        user_id: String,
        /// Shell (shuttle) command to run as other user
        cmd: Vec<String>,
    },

    /// Delete a user
    DeleteUser {
        user_id: String,
    },

    /// Get everything in a user account
    Everything {
        /// user id / project id / email
        query: String,
    },
}
