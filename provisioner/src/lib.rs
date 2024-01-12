use std::ops::Deref;
use std::time::Duration;

pub use args::Args;
use aws_config::timeout;
use aws_sdk_rds::{
    error::SdkError, operation::modify_db_instance::ModifyDBInstanceError, types::DbInstance,
    Client,
};
use error::AuthClientError;
pub use error::Error;
use mongodb::{bson::doc, options::ClientOptions};
use rand::Rng;
use shuttle_common::backends::auth::VerifyClaim;
use shuttle_common::backends::subscription::{NewSubscriptionItem, SubscriptionItem};
use shuttle_common::claims::{AccountTier, Scope};
use shuttle_common::models::project::ProjectName;
pub use shuttle_proto::provisioner::provisioner_server::ProvisionerServer;
use shuttle_proto::provisioner::{
    aws_rds, database_request::DbType, shared, AwsRds, DatabaseRequest, DatabaseResponse, Shared,
};
use shuttle_proto::provisioner::{provisioner_server::Provisioner, DatabaseDeletionResponse};
use shuttle_proto::provisioner::{ContainerRequest, ContainerResponse, Ping, Pong};
use sqlx::{postgres::PgPoolOptions, ConnectOptions, Executor, PgPool};
use tokio::time::sleep;
use tonic::transport::Uri;
use tonic::{Request, Response, Status};
use tracing::{debug, error, info, warn};

mod args;
mod error;

const AWS_RDS_CLASS: &str = "db.t4g.micro";
const MASTER_USERNAME: &str = "master";
const RDS_SUBNET_GROUP: &str = "shuttle_rds";

pub struct ShuttleProvisioner {
    pool: PgPool,
    rds_client: aws_sdk_rds::Client,
    mongodb_client: mongodb::Client,
    fqdn: String,
    internal_pg_address: String,
    internal_mongodb_address: String,
    auth_client: reqwest::Client,
    auth_uri: Uri,
}

impl ShuttleProvisioner {
    pub async fn new(
        shared_pg_uri: &str,
        shared_mongodb_uri: &str,
        fqdn: String,
        internal_pg_address: String,
        internal_mongodb_address: String,
        auth_uri: Uri,
    ) -> Result<Self, Error> {
        let pool = PgPoolOptions::new()
            .min_connections(4)
            .max_connections(12)
            .acquire_timeout(Duration::from_secs(60))
            .connect_lazy(shared_pg_uri)?;

        let mongodb_options = ClientOptions::parse(shared_mongodb_uri).await?;
        let mongodb_client = mongodb::Client::with_options(mongodb_options)?;

        // Default timeout is too long so lowering it
        let timeout_config = timeout::TimeoutConfig::builder()
            .operation_timeout(Duration::from_secs(120))
            .operation_attempt_timeout(Duration::from_secs(120))
            .build();

        let aws_config = aws_config::from_env()
            .timeout_config(timeout_config)
            .load()
            .await;

        let rds_client = aws_sdk_rds::Client::new(&aws_config);

        Ok(Self {
            pool,
            rds_client,
            mongodb_client,
            fqdn,
            internal_pg_address,
            internal_mongodb_address,
            auth_client: reqwest::Client::new(),
            auth_uri,
        })
    }

    pub async fn request_shared_db(
        &self,
        project_name: &str,
        engine: shared::Engine,
    ) -> Result<DatabaseResponse, Error> {
        match engine {
            shared::Engine::Postgres(_) => {
                let (username, password) = self.shared_pg_role(project_name).await?;
                let database_name = self.shared_pg(project_name, &username).await?;

                Ok(DatabaseResponse {
                    engine: "postgres".to_string(),
                    username,
                    password,
                    database_name,
                    address_private: self.internal_pg_address.clone(),
                    address_public: self.fqdn.clone(),
                    port: "5432".to_string(),
                })
            }
            shared::Engine::Mongodb(_) => {
                let database_name = format!("mongodb-{project_name}");
                let (username, password) =
                    self.shared_mongodb(project_name, &database_name).await?;

                Ok(DatabaseResponse {
                    engine: "mongodb".to_string(),
                    username,
                    password,
                    database_name,
                    address_private: self.internal_mongodb_address.clone(),
                    address_public: self.fqdn.clone(),
                    port: "27017".to_string(),
                })
            }
        }
    }

    async fn shared_pg_role(&self, project_name: &str) -> Result<(String, String), Error> {
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

    async fn shared_pg(&self, project_name: &str, username: &str) -> Result<String, Error> {
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

            // Make sure database can't see other databases or other users
            // For #557
            let options = self
                .pool
                .connect_options()
                .deref()
                .clone()
                .database(&database_name);

            let mut conn = options.connect().await?;

            let stmts = vec![
                "REVOKE ALL ON pg_user FROM public;",
                "REVOKE ALL ON pg_roles FROM public;",
                "REVOKE ALL ON pg_database FROM public;",
            ];

            for stmt in stmts {
                conn.execute(stmt)
                    .await
                    .map_err(|e| Error::CreateDB(e.to_string()))?;
            }
        }

        Ok(database_name)
    }

    async fn shared_mongodb(
        &self,
        project_name: &str,
        database_name: &str,
    ) -> Result<(String, String), Error> {
        let username = format!("user-{project_name}");
        let password = generate_password();

        // Get a handle to the DB, create it if it doesn't exist
        let db = self.mongodb_client.database(database_name);

        // Create a new user if it doesn't already exist and assign them
        // permissions to read and write to their own database only
        let new_user = doc! {
            "createUser": &username,
            "pwd": &password,
            "roles": [
                {"role": "readWrite", "db": database_name}
            ]
        };
        let result = db.run_command(new_user, None).await;

        match result {
            Ok(_) => {
                info!("new user created");
                Ok((username, password))
            }
            Err(e) => {
                // If user already exists (error code: 51003) cycle their password
                if e.to_string().contains("51003") {
                    info!("cycling password of user");

                    let change_password = doc! {
                        "updateUser": &username,
                        "pwd": &password,
                    };
                    db.run_command(change_password, None).await?;

                    Ok((username, password))
                } else {
                    Err(Error::UnexpectedMongodb(e))
                }
            }
        }
    }

    async fn request_aws_rds(
        &self,
        project_name: &str,
        engine: &aws_rds::Engine,
    ) -> Result<(bool, DatabaseResponse), Error> {
        let client = &self.rds_client;

        let password = generate_password();
        let instance_name = format!("{}-{}", project_name, engine);

        debug!("trying to get AWS RDS instance: {instance_name}");
        let instance = client
            .modify_db_instance()
            .db_instance_identifier(&instance_name)
            .master_user_password(&password)
            .send()
            .await;

        let mut created_new_instance = false;
        match instance {
            Ok(_) => {
                wait_for_instance(client, &instance_name, "resetting-master-credentials").await?;
            }
            Err(SdkError::ServiceError(err)) => {
                if let ModifyDBInstanceError::DbInstanceNotFoundFault(_) = err.err() {
                    debug!("creating new AWS RDS {instance_name}");

                    // The engine display impl is used for both the engine and the database name,
                    // but for mysql the engine name is an invalid database name.
                    let db_name = if let aws_rds::Engine::Mysql(_) = engine {
                        "msql".to_string()
                    } else {
                        engine.to_string()
                    };

                    client
                        .create_db_instance()
                        .db_instance_identifier(&instance_name)
                        .master_username(MASTER_USERNAME)
                        .master_user_password(&password)
                        .engine(engine.to_string())
                        .db_instance_class(AWS_RDS_CLASS)
                        .allocated_storage(20)
                        .backup_retention_period(0) // Disable backups
                        .publicly_accessible(true)
                        .db_name(db_name)
                        .set_db_subnet_group_name(Some(RDS_SUBNET_GROUP.to_string()))
                        .send()
                        .await?
                        .db_instance
                        .expect("to be able to create instance");

                    wait_for_instance(client, &instance_name, "creating").await?;

                    created_new_instance = true;
                } else {
                    return Err(Error::Plain(format!(
                        "got unexpected error from AWS RDS service: {}",
                        err.err()
                    )));
                }
            }
            Err(unexpected) => {
                return Err(Error::Plain(format!(
                    "got unexpected error from AWS during API call: {}",
                    unexpected
                )))
            }
        };

        // Wait for up
        let instance = wait_for_instance(client, &instance_name, "available").await?;

        // TODO: find private IP somehow
        let address = instance
            .endpoint
            .expect("instance to have an endpoint")
            .address
            .expect("endpoint to have an address");

        Ok((
            created_new_instance,
            DatabaseResponse {
                engine: engine.to_string(),
                username: instance
                    .master_username
                    .expect("instance to have a username"),
                password,
                database_name: instance
                    .db_name
                    .expect("instance to have a default database"),
                address_private: address.clone(),
                address_public: address,
                port: engine_to_port(engine),
            },
        ))
    }

    /// Send a request to the auth service with new subscription items that should be added to
    /// the subscription of the [Claim] subject.
    pub async fn add_subscription_items(
        &self,
        jwt: &str,
        subscription_item: NewSubscriptionItem,
    ) -> Result<(), AuthClientError> {
        let response = self
            .auth_client
            .post(format!("{}users/subscription/items", self.auth_uri))
            .bearer_auth(jwt)
            .json(&subscription_item)
            .send()
            .await
            .map_err(|err| {
                error!(error = %err, "failed to connect to auth service");
                AuthClientError::Internal("failed to connect to auth service".to_string())
            })?;

        match response.status().as_u16() {
            200 => Ok(()),
            499 => {
                error!(
                    status_code = 499,
                    "failed to update subscription due to expired jwt"
                );
                Err(AuthClientError::ExpiredJwt)
            }
            status_code => {
                error!(status_code = status_code, "failed to update subscription");
                Err(AuthClientError::Internal(
                    "failed to update subscription".to_string(),
                ))
            }
        }
    }

    async fn delete_shared_db(
        &self,
        project_name: &str,
        engine: shared::Engine,
    ) -> Result<DatabaseDeletionResponse, Error> {
        match engine {
            shared::Engine::Postgres(_) => self.delete_shared_postgres(project_name).await?,
            shared::Engine::Mongodb(_) => self.delete_shared_mongodb(project_name).await?,
        }
        Ok(DatabaseDeletionResponse {})
    }

    async fn delete_shared_postgres(&self, project_name: &str) -> Result<(), Error> {
        let database_name = format!("db-{project_name}");
        let role_name = format!("user-{project_name}");

        if sqlx::query("SELECT 1 FROM pg_database WHERE datname = $1")
            .bind(&database_name)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| Error::DeleteDB(e.to_string()))?
            .is_some()
        {
            // Identifiers cannot be used as query parameters.
            let drop_db_query = format!("DROP DATABASE \"{database_name}\" WITH (FORCE)");

            // Drop the database with force, which will try to terminate existing connections to the
            // database. This can fail if prepared transactions, active logical replication slots or
            // subscriptions are present in the database.
            sqlx::query(&drop_db_query)
                .execute(&self.pool)
                .await
                .map_err(|e| Error::DeleteDB(e.to_string()))?;

            info!("dropped shared postgres database: {database_name}");
        } else {
            warn!("did not drop shared postgres database: {database_name}. Does not exist.");
        }

        if sqlx::query("SELECT 1 FROM pg_roles WHERE rolname = $1")
            .bind(&role_name)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| Error::DeleteRole(e.to_string()))?
            .is_some()
        {
            // Drop the role.
            let drop_role_query = format!("DROP ROLE IF EXISTS \"{role_name}\"");
            sqlx::query(&drop_role_query)
                .execute(&self.pool)
                .await
                .map_err(|e| Error::DeleteRole(e.to_string()))?;

            info!("dropped shared postgres role: {role_name}");
        } else {
            warn!("did not drop shared postgres role: {role_name}. Does not exist.");
        }

        Ok(())
    }

    async fn delete_shared_mongodb(&self, project_name: &str) -> Result<(), Error> {
        let database_name = format!("mongodb-{project_name}");
        let db = self.mongodb_client.database(&database_name);

        // Dropping a database in mongodb doesn't delete any associated users
        // so do that first.
        let drop_users_command = doc! {
            "dropAllUsersFromDatabase": 1
        };

        db.run_command(drop_users_command, None)
            .await
            .map_err(|e| Error::DeleteRole(e.to_string()))?;

        info!("dropped users from shared mongodb database: {database_name}");

        // Drop the actual database.
        db.drop(None)
            .await
            .map_err(|e| Error::DeleteDB(e.to_string()))?;

        info!("dropped shared mongodb database: {database_name}");

        Ok(())
    }

    async fn delete_aws_rds(
        &self,
        project_name: &str,
        engine: &aws_rds::Engine,
    ) -> Result<DatabaseDeletionResponse, Error> {
        let client = &self.rds_client;
        let instance_name = format!("{project_name}-{engine}");

        // Try to delete the db instance.
        client
            .delete_db_instance()
            .skip_final_snapshot(true)
            .db_instance_identifier(&instance_name)
            .send()
            .await?;

        info!("deleted database instance: {instance_name}");

        Ok(DatabaseDeletionResponse {})
    }
}

#[tonic::async_trait]
impl Provisioner for ShuttleProvisioner {
    #[tracing::instrument(skip(self))]
    async fn provision_database(
        &self,
        request: Request<DatabaseRequest>,
    ) -> Result<Response<DatabaseResponse>, Status> {
        request.verify(Scope::ResourcesWrite)?;

        let can_provision_rds = request.verify_rds_access();

        let request = request.into_inner();
        if !ProjectName::is_valid(&request.project_name) {
            return Err(Status::invalid_argument("invalid project name"));
        }
        let db_type = request.db_type.unwrap();

        let reply = match db_type {
            DbType::Shared(Shared { engine }) => {
                self.request_shared_db(&request.project_name, engine.expect("engine to be set"))
                    .await?
            }
            DbType::AwsRds(AwsRds { engine }) => {
                let claim = can_provision_rds?;

                let engine = engine.expect("engine should be set");

                let (created_new_instance, database_response) =
                    self.request_aws_rds(&request.project_name, &engine).await?;

                // Skip updating subscriptions for admin users, and only update subscription if the
                // rds instance is new.
                if claim.tier != AccountTier::Admin && created_new_instance {
                    // If the subscription update fails, e.g. due to a JWT expiring or the subject's
                    // subscription expiring, delete the instance immediately.
                    if let Err(err) = self
                        .add_subscription_items(
                            // The token should be set on the claim in the JWT auth layer.
                            claim.token().expect("claim should have a token"),
                            NewSubscriptionItem::new(SubscriptionItem::AwsRds, 1),
                        )
                        .await
                    {
                        self.delete_aws_rds(&request.project_name, &engine).await?;

                        return Err(Status::internal(err.to_string()));
                    };
                }

                database_response
            }
        };

        Ok(Response::new(reply))
    }

    #[tracing::instrument(skip(self))]
    async fn delete_database(
        &self,
        request: Request<DatabaseRequest>,
    ) -> Result<Response<DatabaseDeletionResponse>, Status> {
        request.verify(Scope::ResourcesWrite)?;

        let request = request.into_inner();
        if !ProjectName::is_valid(&request.project_name) {
            return Err(Status::invalid_argument("invalid project name"));
        }
        let db_type = request.db_type.unwrap();

        let reply = match db_type {
            DbType::Shared(Shared { engine }) => {
                self.delete_shared_db(&request.project_name, engine.expect("engine to be set"))
                    .await?
            }
            DbType::AwsRds(AwsRds { engine }) => {
                self.delete_aws_rds(&request.project_name, &engine.expect("engine to be set"))
                    .await?
            }
        };

        Ok(Response::new(reply))
    }

    #[tracing::instrument(skip(self))]
    async fn provision_arbitrary_container(
        &self,
        _request: Request<ContainerRequest>,
    ) -> Result<Response<ContainerResponse>, Status> {
        // Intended for use in local runs
        Err(Status::unimplemented(
            "Provisioning arbitrary containers on Shuttle is not supported",
        ))
    }

    #[tracing::instrument(skip(self))]
    async fn health_check(&self, _request: Request<Ping>) -> Result<Response<Pong>, Status> {
        Ok(Response::new(Pong {}))
    }
}

fn generate_password() -> String {
    rand::thread_rng()
        .sample_iter(&rand::distributions::Alphanumeric)
        .take(12)
        .map(char::from)
        .collect()
}

async fn wait_for_instance(
    client: &Client,
    name: &str,
    wait_for: &str,
) -> Result<DbInstance, Error> {
    debug!("waiting for {name} to enter {wait_for} state");
    loop {
        let instance = client
            .describe_db_instances()
            .db_instance_identifier(name)
            .send()
            .await?
            .db_instances
            .expect("aws to return instances")
            .first()
            .expect("to find the instance just created or modified")
            .clone();

        let status = instance
            .db_instance_status
            .as_ref()
            .expect("instance to have a status")
            .clone();

        if status == wait_for {
            return Ok(instance);
        }

        sleep(Duration::from_secs(1)).await;
    }
}

fn engine_to_port(engine: &aws_rds::Engine) -> String {
    match engine {
        aws_rds::Engine::Postgres(_) => "5432".to_string(),
        aws_rds::Engine::Mariadb(_) => "3306".to_string(),
        aws_rds::Engine::Mysql(_) => "3306".to_string(),
    }
}
