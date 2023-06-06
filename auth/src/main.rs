use clap::Parser;
use shuttle_common::backends::tracing::setup_tracing;
use sqlx::migrate::Migrator;
use tracing::trace;

use shuttle_auth::{start, Args, Commands, Sqlite};

pub static MIGRATIONS: Migrator = sqlx::migrate!("./migrations");

#[tokio::main]
async fn main() {
    let args = Args::parse();

    trace!(args = ?args, "parsed args");

    setup_tracing(tracing_subscriber::registry(), "auth");

    let sqlite = Sqlite::new(&args.state.display().to_string()).await;

    match args.command {
        Commands::Start(args) => start(sqlite, args).await,
        Commands::Init(args) => sqlite.insert_admin(&args.name, args.key.as_deref()).await,
    }
}
