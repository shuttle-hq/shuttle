mod api;
mod args;
mod error;
mod secrets;
mod user;

use std::io;

use args::{StartArgs, SyncArgs};
use http::StatusCode;
use shuttle_backends::client::{permit, PermissionsDal};
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
        .with_permissions_client(permit::Client::new(
            args.permit.permit_api_uri,
            args.permit.permit_pdp_uri,
            "default".to_string(),
            args.permit.permit_env,
            args.permit.permit_api_key,
        ))
        .with_jwt_signing_private_key(args.jwt_signing_private_key)
        .into_router();

    info!(address=%args.address, "Binding to and listening at address");

    serve(router, args.address).await;

    Ok(())
}

pub async fn sync(pool: PgPool, args: SyncArgs) -> io::Result<()> {
    let users: Vec<User> = sqlx::query_as("SELECT * FROM users")
        .fetch_all(&pool)
        .await
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

    let permit_client = permit::Client::new(
        args.permit.permit_api_uri,
        args.permit.permit_pdp_uri,
        "default".to_string(),
        args.permit.permit_env,
        &args.permit.permit_api_key,
    );

    for user in users {
        match permit_client.get_user(&user.id).await {
            Ok(p_user) => {
                // Update tier if out of sync
                if !p_user.roles.contains(&user.account_tier) {
                    match user.account_tier {
                        AccountTier::Basic
                        | AccountTier::PendingPaymentPro
                        | AccountTier::CancelledPro
                        | AccountTier::Team
                        | AccountTier::Admin
                        | AccountTier::Deployer => {
                            permit_client
                                .make_free(&user.id)
                                .await
                                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
                        }
                        AccountTier::Pro => {
                            permit_client
                                .make_pro(&user.id)
                                .await
                                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
                        }
                    }
                }
            }
            Err(shuttle_backends::client::Error::RequestError(StatusCode::NOT_FOUND)) => {
                println!("creating user: {}", user.id);

                // Add users that are not in permit
                permit_client
                    .new_user(&user.id)
                    .await
                    .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

                if user.account_tier == AccountTier::Pro {
                    permit_client
                        .make_pro(&user.id)
                        .await
                        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
                }
            }
            Err(e) => return Err(io::Error::new(io::ErrorKind::Other, e)),
        }
    }

    Ok(())
}

pub async fn init(pool: PgPool, args: InitArgs, tier: AccountTier) -> io::Result<()> {
    let key = match args.key {
        Some(ref key) => ApiKey::parse(key).unwrap(),
        None => ApiKey::generate(),
    };

    query("INSERT INTO users (account_name, key, account_tier, user_id) VALUES ($1, $2, $3, $4)")
        .bind("")
        .bind(&key)
        .bind(tier.to_string())
        .bind(&args.user_id)
        .execute(&pool)
        .await
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

    println!(
        "`{}` created as {} with key: {}",
        args.user_id,
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

    Ok(pool)
}
