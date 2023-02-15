use std::str::FromStr;

use clap::Parser;
use opentelemetry::global;
use shuttle_auth::{start, Args};
use sqlx::migrate::{MigrateDatabase, Migrator};
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqliteSynchronous};
use sqlx::{Sqlite, SqlitePool};
use tracing::{info, trace};
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

pub static MIGRATIONS: Migrator = sqlx::migrate!("./migrations");

#[tokio::main]
async fn main() {
    let args = Args::parse();

    trace!(args = ?args, "parsed args");

    global::set_text_map_propagator(opentelemetry_datadog::DatadogPropagator::new());

    let fmt_layer = fmt::layer();
    let filter_layer = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new("info"))
        .unwrap();

    let tracer = opentelemetry_datadog::new_pipeline()
        .with_service_name("gateway")
        .with_agent_endpoint("http://datadog-agent:8126")
        .install_batch(opentelemetry::runtime::Tokio)
        .unwrap();
    let opentelemetry = tracing_opentelemetry::layer().with_tracer(tracer);
    tracing_subscriber::registry()
        .with(filter_layer)
        .with(fmt_layer)
        .with(opentelemetry)
        .init();

    let db_path = args.state.join("authentication.sqlite");
    let db_uri = db_path.to_str().unwrap();

    if !db_path.exists() {
        Sqlite::create_database(db_uri).await.unwrap();
    }

    info!(
        "state db: {}",
        std::fs::canonicalize(&args.state)
            .unwrap()
            .to_string_lossy()
    );

    // https://phiresky.github.io/blog/2020/sqlite-performance-tuning/
    let sqlite_options = SqliteConnectOptions::from_str(db_uri)
        .unwrap()
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Normal);

    let db = SqlitePool::connect_with(sqlite_options).await.unwrap();

    MIGRATIONS.run(&db).await.unwrap();

    start(args, db).await;
}
