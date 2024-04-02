mod api;
mod args;
mod error;
mod secrets;
mod user;

use anyhow::Result;
use args::{CopyPermitEnvArgs, StartArgs, SyncArgs};
use shuttle_backends::client::{permit, PermissionsDal};
use shuttle_common::{claims::AccountTier, ApiKey};
use sqlx::{migrate::Migrator, query, PgPool};
use tracing::info;
pub use user::User;

use crate::api::serve;
pub use api::ApiBuilder;
pub use args::{Args, Commands, InitArgs};

pub static MIGRATIONS: Migrator = sqlx::migrate!("./migrations");

pub async fn start(pool: PgPool, args: StartArgs) {
    let router = api::ApiBuilder::new()
        .with_pg_pool(pool)
        .with_stripe_client(stripe::Client::new(args.stripe_secret_key))
        .with_permissions_client(permit::Client::new(
            args.permit.permit_api_uri.to_string(),
            args.permit.permit_pdp_uri.to_string(),
            "default".to_string(),
            args.permit.permit_env,
            args.permit.permit_api_key,
        ))
        .with_jwt_signing_private_key(args.jwt_signing_private_key)
        .into_router();

    info!(address=%args.address, "Binding to and listening at address");

    serve(router, args.address).await;
}

pub async fn sync(pool: PgPool, args: SyncArgs) -> Result<()> {
    let users: Vec<User> = sqlx::query_as("SELECT * FROM users")
        .fetch_all(&pool)
        .await?;

    let permit_client = permit::Client::new(
        args.permit.permit_api_uri.to_string(),
        args.permit.permit_pdp_uri.to_string(),
        "default".to_string(),
        args.permit.permit_env,
        args.permit.permit_api_key,
    );

    for user in users {
        match permit_client.get_user(&user.id).await {
            Ok(p_user) => {
                // Update tier if out of sync
                if !p_user
                    .roles
                    .is_some_and(|rs| rs.iter().any(|r| r.role == user.account_tier.to_string()))
                {
                    match user.account_tier {
                        AccountTier::Basic
                        | AccountTier::PendingPaymentPro
                        | AccountTier::CancelledPro
                        | AccountTier::Team
                        | AccountTier::Admin
                        | AccountTier::Deployer => {
                            permit_client.make_basic(&user.id).await?;
                        }
                        AccountTier::Pro => {
                            permit_client.make_pro(&user.id).await?;
                        }
                    }
                }
            }
            Err(_) => {
                // FIXME: Make the error type better so that this is only done on 404s

                println!("creating user: {}", user.id);

                // Add users that are not in permit
                permit_client.new_user(&user.id).await?;

                if user.account_tier == AccountTier::Pro {
                    permit_client.make_pro(&user.id).await?;
                }
            }
        }
    }

    Ok(())
}

pub async fn copy_environment(args: CopyPermitEnvArgs) -> Result<()> {
    let client = permit::Client::new(
        args.permit.permit_api_uri.to_string(),
        args.permit.permit_pdp_uri.to_string(),
        "default".to_string(),
        args.permit.permit_env,
        args.permit.permit_api_key,
    );

    client.copy_environment(&args.target).await
}

pub async fn init(pool: PgPool, args: InitArgs, tier: AccountTier) -> Result<()> {
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
        .await?;

    println!(
        "`{}` created as {} with key: {}",
        args.user_id,
        tier,
        key.as_ref()
    );
    Ok(())
}

/// Initialize the connection pool to a Postgres database at the given URI.
pub async fn pgpool_init(db_uri: &str) -> Result<PgPool> {
    let opts = db_uri.parse()?;
    let pool = PgPool::connect_with(opts).await?;
    MIGRATIONS.run(&pool).await?;

    Ok(pool)
}
