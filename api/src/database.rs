use lazy_static::lazy_static;
use lib::DatabaseReadyInfo;
use rand::Rng;
use sqlx::postgres::{PgPool, PgPoolOptions};

use lib::project::ProjectConfig;

lazy_static! {
    static ref SUDO_POSTGRES_CONNECTION_STRING: String = format!(
        "postgres://postgres:{}@localhost",
        std::env::var("PG_PASSWORD").expect(
            "superuser postgres role password expected as environment variable PG_PASSWORD"
        )
    );
}

fn generate_role_password() -> String {
    rand::thread_rng()
        .sample_iter(&rand::distributions::Alphanumeric)
        .take(12)
        .map(char::from)
        .collect()
}

pub(crate) struct State {
    project: ProjectConfig,
    context: Context,
    is_guaranteed: bool,
    info: Option<DatabaseReadyInfo>
}

impl State {
    pub(crate) fn new(project: &ProjectConfig, context: &Context) -> Self {
        Self {
            project: project.clone(),
            context: context.clone(),
            is_guaranteed: false,
            info: None
        }
    }

    pub(crate) fn request(&mut self) -> DatabaseReadyInfo {
        if self.info.is_none() {
            let role_name = format!("user-{}", self.project.name());
            let role_password = generate_role_password();
            let database_name = format!("db-{}", self.project.name());
            let info = DatabaseReadyInfo::new(role_name, role_password, database_name);
            self.info = Some(info.clone());
            info
        } else {
            self.info.clone().unwrap()
        }
    }

    pub(crate) async fn ensure(&mut self) -> sqlx::Result<()> {
        if self.info.is_none() || self.is_guaranteed {
            return Ok(())
        }

        let DatabaseReadyInfo { role_name, role_password, database_name } = self.info.clone().unwrap();

        let pool = &self.context.sudo_pool;

        // Check if this deployment already has its own role:
        let rows = sqlx::query("SELECT * FROM pg_roles WHERE rolname = $1")
            .bind(&role_name)
            .fetch_all(pool)
            .await?;

        if rows.is_empty() {
            // Create role if it does not already exist:
            // TODO: Should be able to use `.bind` instead of `format!` but doesn't seem to
            // insert quotes correctly.
            let create_role_query = format!(
                "CREATE ROLE \"{}\" PASSWORD '{}' LOGIN",
                role_name, role_password
            );
            sqlx::query(&create_role_query)
                .execute(pool)
                .await?;

            debug!(
                "created new role '{}' in database for project '{}'",
                role_name,
                database_name
            );
        } else {
            // If the role already exists then change its password:
            let alter_password_query = format!(
                "ALTER ROLE \"{}\" WITH PASSWORD '{}'",
                role_name, role_password
            );
            sqlx::query(&alter_password_query)
                .execute(pool)
                .await?;

            debug!(
                "role '{}' already exists so updating their password",
                role_name
            );
        }

        // Since user creation is not atomic, need to separately check for DB existence
        let get_database_query = "SELECT 1 FROM pg_database WHERE datname = $1";
        let database = sqlx::query(get_database_query)
            .bind(&database_name)
            .fetch_all(pool)
            .await?;
        if database.is_empty() {
            debug!("database '{}' does not exist, creating", database_name);
            // Create the database (owned by the new role):
            let create_database_query = format!(
                "CREATE DATABASE \"{}\" OWNER '{}'",
                database_name, role_name
            );
            sqlx::query(&create_database_query)
                .execute(pool)
                .await?;

            debug!(
                "created database '{}' belonging to '{}'",
                database_name,
                role_name
            );
        } else {
            debug!("database '{}' already exists, not recreating", database_name);
        }

        self.is_guaranteed = true;

        Ok(())
    }
}


#[derive(Clone)]
pub struct Context {
    sudo_pool: PgPool,
}

impl Context {
    pub async fn new() -> sqlx::Result<Self> {
        Ok(Context {
            sudo_pool: PgPoolOptions::new()
                .min_connections(4)
                .max_connections(12)
                .connect_lazy(&SUDO_POSTGRES_CONNECTION_STRING)?,
        })
    }
}
