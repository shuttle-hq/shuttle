use std::time::Duration;

pub use args::Args;
pub use error::Error;
use proto::provisioner::provisioner_server::Provisioner;
pub use proto::provisioner::provisioner_server::ProvisionerServer;
use proto::provisioner::{DatabaseRequest, DatabaseResponse};
use rand::Rng;
use sqlx::{postgres::PgPoolOptions, PgPool};
use tonic::{Request, Response, Status};
use tracing::info;

mod args;
mod error;

pub struct MyProvisioner {
    pool: PgPool,
}

impl MyProvisioner {
    pub fn new(uri: &str) -> sqlx::Result<Self> {
        Ok(Self {
            pool: PgPoolOptions::new()
                .min_connections(4)
                .max_connections(12)
                .connect_timeout(Duration::from_secs(60))
                .connect_lazy(uri)?,
        })
    }

    pub async fn request_shared_db(&self, project_name: &str) -> Result<DatabaseResponse, Error> {
        let (username, password) = self.shared_role(project_name).await?;
        let database_name = self.shared_db(project_name, &username).await?;

        Ok(DatabaseResponse {
            username,
            password,
            database_name,
        })
    }

    async fn shared_role(&self, project_name: &str) -> Result<(String, String), Error> {
        let username = format!("user-{project_name}");
        let password = generate_password();

        let matching_user = sqlx::query("SELECT rolname FROM pg_roles WHERE rolname = $1")
            .bind(&username)
            .fetch_optional(&self.pool)
            .await?;

        if matching_user.is_none() {
            info!("creating new user");

            // Binding does not work for identifiers
            // https://stackoverflow.com/questions/63723236/sql-statement-to-create-role-fails-on-postgres-12-using-dapper
            let create_role_query =
                format!("CREATE ROLE \"{username}\" WITH LOGIN PASSWORD '{password}'");
            sqlx::query(&create_role_query)
                .execute(&self.pool)
                .await
                .map_err(|e| Error::CreateRole(e.to_string()))?;
        } else {
            info!("cycling password of user");

            // Binding does not work for identifiers
            // https://stackoverflow.com/questions/63723236/sql-statement-to-create-role-fails-on-postgres-12-using-dapper
            let update_role_query =
                format!("ALTER ROLE \"{username}\" WITH LOGIN PASSWORD '{password}'");
            sqlx::query(&update_role_query)
                .execute(&self.pool)
                .await
                .map_err(|e| Error::UpdateRole(e.to_string()))?;
        }

        Ok((username, password))
    }

    async fn shared_db(&self, project_name: &str, username: &str) -> Result<String, Error> {
        let database_name = format!("db-{project_name}");

        let matching_db = sqlx::query("SELECT datname FROM pg_database WHERE datname = $1")
            .bind(&database_name)
            .fetch_optional(&self.pool)
            .await?;

        if matching_db.is_none() {
            info!("creating database");

            // Binding does not work for identifiers
            // https://stackoverflow.com/questions/63723236/sql-statement-to-create-role-fails-on-postgres-12-using-dapper
            let create_db_query = format!("CREATE DATABASE \"{database_name}\" OWNER '{username}'");
            sqlx::query(&create_db_query)
                .execute(&self.pool)
                .await
                .map_err(|e| Error::CreateDB(e.to_string()))?;
        }

        Ok(database_name)
    }
}

#[tonic::async_trait]
impl Provisioner for MyProvisioner {
    #[tracing::instrument(skip(self))]
    async fn provision_database(
        &self,
        request: Request<DatabaseRequest>,
    ) -> Result<Response<DatabaseResponse>, Status> {
        let reply = self
            .request_shared_db(&request.into_inner().project_name)
            .await?;

        Ok(Response::new(reply))
    }
}

fn generate_password() -> String {
    rand::thread_rng()
        .sample_iter(&rand::distributions::Alphanumeric)
        .take(12)
        .map(char::from)
        .collect()
}
