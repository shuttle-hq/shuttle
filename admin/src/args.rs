use clap::{Parser, Subcommand};
use shuttle_common::models::{project::ComputeTier, user::AccountTier};

#[derive(Parser, Debug)]
pub struct Args {
    /// Target a different Shuttle API env (use a separate global config) (default: None (= prod = production))
    // ("SHUTTLE_ENV" is used for user-facing environments (agnostic of Shuttle API env))
    #[arg(global = true, long, env = "SHUTTLE_API_ENV")]
    pub api_env: Option<String>,
    /// URL for the Shuttle API to target (overrides inferred URL from api_env)
    #[arg(global = true, long, env = "SHUTTLE_API")]
    pub api_url: Option<String>,

    #[command(subcommand)]
    pub command: Command,

    /// Request timeout for the API client in seconds.
    #[arg(long, default_value_t = 120)]
    pub client_timeout: u64,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    ChangeProjectOwner {
        /// Project to update ownership of
        project_id: String,
        /// User id to switch ownership to
        new_user_id: String,
    },
    AddUserToTeam {
        team_user_id: String,
        user_id: String,
    },

    UpdateProjectConfig {
        /// Project to update
        #[arg(long, visible_alias = "id")]
        project_id: String,
        /// Project configuration as JSON
        #[arg(long, visible_alias = "config")]
        json: String,
    },

    GetProjectConfig {
        /// Project to get config for
        #[arg(long, visible_alias = "id")]
        project_id: String,
    },

    /// Upgrade project to use a dedicated load balancer.
    UpgradeProjectToLb {
        /// Project to upgrade to ALB
        #[arg(long, visible_alias = "id")]
        project_id: String,
    },

    /// Update compute tier for a given project.
    UpdateProjectScale {
        /// Project to update
        #[arg(long, visible_alias = "id")]
        project_id: String,
        /// Compute tier to set for the given project
        #[arg(long, visible_alias = "tier")]
        compute_tier: Option<ComputeTier>,
        /// Replica count to set for the given project
        #[arg(long)]
        replicas: Option<u8>,
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

    /// Delete a user
    DeleteUser {
        user_id: String,
    },

    SetAccountTier {
        user_id: String,
        tier: AccountTier,
    },

    /// Get info about everything in a user account
    Everything {
        /// user id / project id / email
        query: String,
    },

    DowngradeProTrials,
    CleanupCloudmapServices,
}
