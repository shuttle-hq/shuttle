use lazy_static::lazy_static;
use rand::Rng;
use sqlx::pool::PoolConnection;
use sqlx::postgres::{PgPool, PgPoolOptions};

lazy_static! {
    static ref SUDO_POSTGRES_CONNECTION_STRING: String = format!(
        "postgres://postgres:{}@localhost",
        std::env::var("SUDO_POSTGRES_PASSWORD").expect("superuser postgres role password expected as environment variable SUDO_POSTGRES_PASSWORD")
    );
}

fn generate_role_password() -> String {
    rand::thread_rng()
        .sample_iter(&rand::distributions::Alphanumeric)
        .take(12)
        .map(char::from)
        .collect()
}

pub(crate) struct DatabaseResource {
    state: DatabaseState,
    sudo_pool: PgPool, // TODO: share this pool properly
    role_password: String,
}

impl DatabaseResource {
    pub(crate) async fn new() -> sqlx::Result<Self> {
        log::debug!("preparing database resource");
        Ok(Self {
            state: DatabaseState::Uninitialised,
            sudo_pool: PgPoolOptions::new()
                .max_connections(10)
                .connect(&SUDO_POSTGRES_CONNECTION_STRING)
                .await?,
            role_password: generate_role_password(),
        })
    }

    pub(crate) async fn get_client(
        &mut self,
        name: &str,
    ) -> sqlx::Result<PoolConnection<sqlx::Postgres>> {
        log::debug!("getting database client for project '{}'", name);

        let role_name = format!("user-{}", name);
        let database_name = format!("db-{}", name);

        match self.state {
            DatabaseState::Uninitialised => {
                // Check if this deployment already has its own role:

                let rows = sqlx::query("SELECT * FROM pg_roles WHERE rolname = $1")
                    .bind(&role_name)
                    .fetch_all(&self.sudo_pool)
                    .await?;

                if rows.is_empty() {
                    // Create role if it does not already exist:

                    // TODO: Should be able to use `.bind` instead of `format!` but doesn't seem to
                    // insert quotes correctly.
                    let create_role_query = format!(
                        "CREATE ROLE \"{}\" PASSWORD '{}' LOGIN",
                        role_name, self.role_password
                    );
                    sqlx::query(&create_role_query)
                        .execute(&self.sudo_pool)
                        .await?;

                    log::debug!(
                        "created new role '{}' in database for project '{}'",
                        role_name,
                        name
                    );

                    // Create the database (owned by the new role):

                    let create_database_query = format!(
                        "CREATE DATABASE \"{}\" OWNER '{}'",
                        database_name, role_name
                    );
                    sqlx::query(&create_database_query)
                        .execute(&self.sudo_pool)
                        .await?;

                    log::debug!(
                        "created database '{}' belonging to '{}'",
                        database_name,
                        role_name
                    );
                }

                // Create connection pool:

                let connection_string = format!(
                    "postgres://{}:{}@localhost/{}",
                    role_name, self.role_password, database_name
                );
                log::debug!("{}", connection_string);
                let pool = PgPoolOptions::new()
                    .max_connections(10)
                    .connect(&connection_string)
                    .await?;

                let connection = pool.acquire().await;

                // Transition to the 'ready' state:
                self.state = DatabaseState::Ready(pool);

                connection
            }
            DatabaseState::Ready(ref pool) => pool.acquire().await,
        }
    }

    pub(crate) fn role_password(&self) -> String {
        self.role_password.clone()
    }
}

enum DatabaseState {
    Uninitialised,
    Ready(PgPool),
}
