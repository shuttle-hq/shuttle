use rand::Rng;
use sqlx::pool::PoolConnection;
use sqlx::postgres::{PgPool, PgPoolOptions};

const SUDO_POSTGRES_CONNECTION_STR: &str = "postgres://localhost";

fn generate_role_password() -> String {
    rand::thread_rng()
        .sample_iter(&rand::distributions::Alphanumeric)
        .take(15)
        .map(char::from)
        .collect()
}

pub(crate) struct DatabaseResource {
    state: DatabaseState,
    sudo_pool: PgPool, // TODO: share this pool properly
    role_password: String,
}

impl DatabaseResource {
    pub(crate) fn new() -> sqlx::Result<Self> {
        Ok(Self {
            state: DatabaseState::Uninitialised,
            sudo_pool: PgPoolOptions::new()
                .max_connections(10)
                .connect_lazy(SUDO_POSTGRES_CONNECTION_STR)?,
            role_password: generate_role_password(),
        })
    }

    pub(crate) async fn get_client(
        &mut self,
        name: &str,
    ) -> sqlx::Result<PoolConnection<sqlx::Postgres>> {
        let role_name = format!("user-{}", name);
        let database_name = format!("db-{}", name);

        match self.state {
            DatabaseState::Uninitialised => {
                // Check if this deployment already has its own role:

                let rows = sqlx::query("SELECT * FROM pg_roles WHERE rolname=$1")
                    .bind(&role_name)
                    .fetch_all(&self.sudo_pool)
                    .await?;

                if rows.is_empty() {
                    // Create role if it does not already exist:

                    sqlx::query("CREATE ROLE $1 PASSWORD $2 LOGIN")
                        .bind(&role_name)
                        .bind(&self.role_password)
                        .execute(&self.sudo_pool)
                        .await?;
                }

                // Create the database (owned by the new role):

                sqlx::query("CREATE DATABASE $1 OWNER $2")
                    .bind(&database_name)
                    .bind(&role_name)
                    .execute(&self.sudo_pool)
                    .await?;

                // Create connection pool:

                let connection_string = format!(
                    "postgres://{}:{}@localhost/{}",
                    role_name, self.role_password, database_name
                );
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
