use std::io;

use clap::Parser;
use shuttle_common::{backends::tracing::setup_tracing, claims::AccountTier, log::Backend};
use sqlx::migrate::Migrator;
use tracing::trace;

use shuttle_auth::{init, pgpool_init, start, Args, Commands};

pub static MIGRATIONS: Migrator = sqlx::migrate!("./migrations");

#[tokio::main]
async fn main() -> io::Result<()> {
    let args = Args::parse();

    trace!(args = ?args, "parsed args");

    setup_tracing(tracing_subscriber::registry(), Backend::Auth, None);

    let pool = pgpool_init(args.db_connection_uri.as_str())
        .await
        .expect("couldn't setup the postgres connection");

    match args.command {
        Commands::Start(args) => start(pool, args).await,
        Commands::InitAdmin(args) => init(pool, args, AccountTier::Admin).await,
        Commands::InitDeployer(args) => init(pool, args, AccountTier::Deployer).await,
    }
}
