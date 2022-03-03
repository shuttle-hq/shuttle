use sqlx::pool::PoolConnection;
use sqlx::postgres::{PgPool, PgPoolOptions};

use lib::DeploymentId;

const SUDO_POSTGRES_CONNECTION_STR: &str = "postgres://localhost";

pub(crate) struct DatabaseResource {
    state: DatabaseState,
    sudo_pool: PgPool, // TODO: share this pool properly
}

impl DatabaseResource {
    pub(crate) fn new() -> sqlx::Result<Self> {
        Ok(Self {
            state: DatabaseState::Uninitialised,
            sudo_pool: PgPoolOptions::new()
                .max_connections(10)
                .connect_lazy(SUDO_POSTGRES_CONNECTION_STR)?,
        })
    }

    pub(crate) async fn get_client(
        &mut self,
        id: DeploymentId,
    ) -> sqlx::Result<PoolConnection<sqlx::Postgres>> {
        let id_string = id.to_hyphenated().to_string();
        let role_name = format!("user-{}", id_string);
        let role_password = "pa55w0rd".to_string(); // TODO
        let database_name = format!("db-{}", id_string);

        match self.state {
            DatabaseState::Uninitialised => {
                // Check if this deployment already has its own role:

                let rows = sqlx::query("SELECT * FROM pg_roles WHERE rolname=$1")
                    .bind(&role_name)
                    .fetch_all(&self.sudo_pool)
                    .await?;

                if rows.len() == 0 {
                    // Create role if it does not already exist:

                    sqlx::query("CREATE ROLE $1 PASSWORD $2 LOGIN")
                        .bind(&role_name)
                        .bind(&role_password)
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

                let connection_string = format!("postgres://{}:{}@localhost/{}", role_name, role_password, database_name);
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
}

enum DatabaseState {
    Uninitialised,
    Ready(PgPool),
}
