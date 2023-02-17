mod api;
mod args;
mod error;
mod user;

use std::{io, str::FromStr};

use args::StartArgs;
use sqlx::{
    migrate::Migrator,
    query,
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqliteSynchronous},
    SqlitePool,
};
use tracing::info;

use crate::{
    api::serve,
    user::{AccountTier, Key},
};
pub use api::ApiBuilder;
pub use args::{Args, Commands, InitArgs};

pub static MIGRATIONS: Migrator = sqlx::migrate!("./migrations");

pub async fn start(pool: SqlitePool, args: StartArgs) -> io::Result<()> {
    let router = api::ApiBuilder::new().with_sqlite_pool(pool).into_router();

    info!(address=%args.address, "Binding to and listening at address");

    serve(router, args.address).await;

    Ok(())
}

pub async fn init(pool: SqlitePool, args: InitArgs) -> io::Result<()> {
    let key = match args.key {
        Some(ref key) => Key::from_str(key).unwrap(),
        None => Key::new_random(),
    };

    query("INSERT INTO users (account_name, key, account_tier) VALUES (?1, ?2, ?3)")
        .bind(&args.name)
        .bind(&key)
        .bind(AccountTier::Admin)
        .execute(&pool)
        .await
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

    println!("`{}` created as super user with key: {key}", args.name);
    Ok(())
}

/// Initialize an SQLite database at the given URI, creating it if it does not
/// already exist. To create an in-memory database for tests, simply pass in
/// `sqlite::memory:` for the `db_uri`.
pub async fn sqlite_init(db_uri: &str) -> SqlitePool {
    let sqlite_options = SqliteConnectOptions::from_str(db_uri)
        .unwrap()
        .create_if_missing(true)
        // To see the sources for choosing these settings, see:
        // https://github.com/shuttle-hq/shuttle/pull/623
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Normal);

    let pool = SqlitePool::connect_with(sqlite_options).await.unwrap();

    MIGRATIONS.run(&pool).await.unwrap();

    pool
}
