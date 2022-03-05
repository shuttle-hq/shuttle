use lazy_static::lazy_static;
use rand::Rng;
use sqlx::postgres::{PgPool, PgPoolOptions};

lazy_static! {
    static ref SUDO_POSTGRES_CONNECTION_STRING: String = format!(
        "postgres://postgres:{}@localhost",
        std::env::var("PG_PASSWORD").expect("superuser postgres role password expected as environment variable SUDO_POSTGRES_PASSWORD")
    );
}

fn generate_role_password() -> String {
    rand::thread_rng()
        .sample_iter(&rand::distributions::Alphanumeric)
        .take(12)
        .map(char::from)
        .collect()
}

pub(crate) enum State {
    Uninitialised,
    Ready(ReadyState),
}

impl State {
    pub(crate) async fn advance(&mut self, name: &str, ctx: &Context) -> sqlx::Result<ReadyState> {
        match self {
            State::Uninitialised => {
                let role_name = format!("user-{}", name);
                let role_password = generate_role_password();
                let database_name = format!("db-{}", name);

                // Check if this deployment already has its own role:

                let rows = sqlx::query("SELECT * FROM pg_roles WHERE rolname = $1")
                    .bind(&role_name)
                    .fetch_all(&ctx.sudo_pool)
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
                        .execute(&ctx.sudo_pool)
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
                        .execute(&ctx.sudo_pool)
                        .await?;

                    log::debug!(
                        "created database '{}' belonging to '{}'",
                        database_name,
                        role_name
                    );
                }

                // Transition to the 'ready' state:

                let ready = ReadyState {
                    role_name,
                    role_password,
                    database_name,
                };

                *self = State::Ready(ready.clone());

                Ok(ready)
            }
            State::Ready(ref ready) => Ok(ready.clone()),
        }
    }
}

impl Default for State {
    fn default() -> Self {
        State::Uninitialised
    }
}

#[derive(Debug, Clone)]
pub struct ReadyState {
    role_name: String,
    role_password: String,
    database_name: String,
}

impl ReadyState {
    pub fn connection_string(&self) -> String {
        format!(
            "postgres://{}:{}@localhost/{}",
            self.role_name, self.role_password, self.database_name
        )
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
                .max_connections(10)
                .connect_lazy(&SUDO_POSTGRES_CONNECTION_STRING)?,
        })
    }
}
