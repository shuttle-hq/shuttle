mod api;
mod args;
mod error;
mod secrets;
mod user;

use std::io;

use args::StartArgs;
use shuttle_common::{claims::AccountTier, ApiKey};
use sqlx::{migrate::Migrator, query, PgPool};
use tracing::info;
pub use user::User;

use crate::api::serve;
pub use api::ApiBuilder;
pub use args::{Args, Commands, InitArgs};

pub static MIGRATIONS: Migrator = sqlx::migrate!("./migrations");

pub async fn start(pool: PgPool, args: StartArgs) -> io::Result<()> {
    let router = api::ApiBuilder::new()
        .with_pg_pool(pool)
        .with_stripe_client(stripe::Client::new(args.stripe_secret_key))
        .with_jwt_signing_private_key(args.jwt_signing_private_key)
        .into_router();

    info!(address=%args.address, "Binding to and listening at address");

    serve(router, args.address).await;

    Ok(())
}

pub async fn init(pool: PgPool, args: InitArgs, tier: AccountTier) -> io::Result<()> {
    let key = match args.key {
        Some(ref key) => ApiKey::parse(key).unwrap(),
        None => ApiKey::generate(),
    };

    query("INSERT INTO users (account_name, key, account_tier) VALUES ($1, $2, $3)")
        .bind(&args.name)
        .bind(&key)
        .bind(tier.to_string())
        .execute(&pool)
        .await
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

    println!(
        "`{}` created as {} with key: {}",
        args.name,
        tier,
        key.as_ref()
    );
    Ok(())
}

/// Initialize the connection pool to a Postgres database at the given URI.
pub async fn pgpool_init(db_uri: &str) -> io::Result<PgPool> {
    let opts = db_uri
        .parse()
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
    let pool = PgPool::connect_with(opts)
        .await
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
    MIGRATIONS
        .run(&pool)
        .await
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

    // Post-migration logic for 0003.
    // This is done here to skip the need for postgres extensions.
    let names: Vec<(String,)> =
        sqlx::query_as("SELECT account_name FROM users WHERE user_id IS NULL")
            .fetch_all(&pool)
            .await
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
    for (name,) in names {
        sqlx::query("UPDATE users SET user_id = $1 WHERE account_name = $2")
            .bind(User::new_user_id())
            .bind(name)
            .execute(&pool)
            .await
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
    }

    Ok(pool)
}
