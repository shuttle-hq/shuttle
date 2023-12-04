mod api;
mod args;
mod error;
mod secrets;
mod user;

use std::{io, time::Duration};

use args::StartArgs;
use shuttle_common::{claims::AccountTier, ApiKey};
use sqlx::{migrate::Migrator, query, PgPool};
use tracing::info;

use crate::api::serve;
pub use api::ApiBuilder;
pub use args::{Args, Commands, InitArgs};

pub const COOKIE_EXPIRATION: Duration = Duration::from_secs(60 * 60 * 24); // One day

pub static MIGRATIONS: Migrator = sqlx::migrate!("./migrations");

pub async fn start(pool: PgPool, args: StartArgs) -> io::Result<()> {
    let router = api::ApiBuilder::new()
        .with_pg_pool(pool)
        .with_sessions()
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
        .bind(tier)
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
    MIGRATIONS.run(&pool).await.unwrap();

    Ok(pool)
}
