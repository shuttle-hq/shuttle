use std::time::Duration;

pub use args::Args;
use aws_config::timeout;
use aws_sdk_elasticache::error::ModifyCacheClusterErrorKind;
use aws_sdk_elasticache::model::CacheCluster;
use aws_sdk_rds::{error::ModifyDBInstanceErrorKind, model::DbInstance, types::SdkError, Client};
pub use error::Error;
use mongodb::{bson::doc, options::ClientOptions};
use rand::Rng;
pub use shuttle_proto::provisioner::provisioner_server::ProvisionerServer;
use shuttle_proto::provisioner::{
    aws_rds, database_request::DbType, elasti_cache, shared, AwsRds, DatabaseRequest,
    DatabaseResponse, Shared,
};
use shuttle_proto::provisioner::{provisioner_server::Provisioner, ElastiCache};
use sqlx::{postgres::PgPoolOptions, PgPool};
use tokio::time::sleep;
use tonic::{Request, Response, Status};
use tracing::{debug, info};

mod args;
mod error;

const AWS_RDS_CLASS: &str = "db.t4g.micro";
const MASTER_USERNAME: &str = "master";
const RDS_SUBNET_GROUP: &str = "shuttle_rds";
const ELASTICACHE_SUBNET_GROUP: &str = "shuttle_elasticache";

pub struct MyProvisioner {
    pool: PgPool,
    elasticache_client: aws_sdk_elasticache::Client,
    rds_client: aws_sdk_rds::Client,
    mongodb_client: mongodb::Client,
    fqdn: String,
    internal_pg_address: String,
    internal_mongodb_address: String,
}

impl MyProvisioner {
    pub async fn new(
        shared_pg_uri: &str,
        shared_mongodb_uri: &str,
        fqdn: String,
        internal_pg_address: String,
        internal_mongodb_address: String,
        internal_redis_address: String,
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

        let elasticache_client = aws_sdk_elasticache::Client::new(&aws_config);

        Ok(Self {
            pool,
            elasticache_client,
            rds_client,
            mongodb_client,
            fqdn,
            internal_pg_address,
            internal_mongodb_address,
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
        engine: aws_rds::Engine,
    ) -> Result<DatabaseResponse, Error> {
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

        match instance {
            Ok(_) => {
                wait_for_instance(client, &instance_name, "resetting-master-credentials").await?;
            }
            Err(SdkError::ServiceError(err)) => {
                if let ModifyDBInstanceErrorKind::DbInstanceNotFoundFault(_) = err.err().kind {
                    debug!("creating new AWS {instance_name}");

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
                        .db_name(engine.to_string())
                        .set_db_subnet_group_name(Some(RDS_SUBNET_GROUP.to_string()))
                        .send()
                        .await?
                        .db_instance
                        .expect("to be able to create instance");

                    wait_for_instance(client, &instance_name, "creating").await?;
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

        Ok(DatabaseResponse {
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
        })
    }

    async fn request_elasticache(
        &self,
        project_name: &str,
        engine: elasti_cache::Engine,
    ) -> Result<DatabaseResponse, Error> {
        let client = &self.elasticache_client;

        let password = generate_password();
        let cluster_name = format!("{}-elasticache", project_name);

        debug!("trying to get AWS ElastiCache cluster: {cluster_name}");
        let cluster = client
            .modify_cache_cluster()
            .cache_cluster_id(&cluster_name)
            .auth_token(&password)
            .send()
            .await;

        match cluster {
            Ok(_) => {
                wait_for_cluster(client, &cluster_name, "resetting-master-credentials").await?;
            }
            Err(SdkError::ServiceError(err)) => {
                if let ModifyCacheClusterErrorKind::CacheClusterNotFoundFault(_) = err.err().kind {
                    debug!("creating new AWS ElastiCache {cluster_name}");

                    client
                        .create_cache_cluster()
                        .cache_cluster_id(&cluster_name)
                        .auth_token(&password)
                        .engine(engine.to_string())
                        .engine_version("redis6.2") // TODO: pass this through with the DB_request as well
                        .cache_node_type(AWS_RDS_CLASS)
                        .num_cache_nodes(1)
                        .cache_subnet_group_name(ELASTICACHE_SUBNET_GROUP)
                        .send()
                        .await?
                        .cache_cluster()
                        .expect("to be able to create cluster");

                    wait_for_cluster(client, &cluster_name, "creating").await?;
                } else {
                    return Err(Error::Plain(format!(
                        "got unexpected error from AWS ElastiCache service: {}",
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
        let cluster = wait_for_cluster(client, &cluster_name, "available").await?;

        let address = cluster
            .configuration_endpoint()
            .expect("cluster to have an endpoint")
            .address()
            .expect("endpoint to have an address");

        Ok(DatabaseResponse {
            engine: "redis".to_owned(),
            username: "redis".to_owned(),
            password,
            database_name: cluster
                .cache_cluster_id()
                .expect("cluster to have an ID")
                .to_owned(),
            address_private: address.to_owned(),
            address_public: address.to_owned(),
            port: "6379".to_owned(),
        })
    }
}

#[tonic::async_trait]
impl Provisioner for MyProvisioner {
    #[tracing::instrument(skip(self))]
    async fn provision_database(
        &self,
        request: Request<DatabaseRequest>,
    ) -> Result<Response<DatabaseResponse>, Status> {
        let request = request.into_inner();
        let db_type = request.db_type.unwrap();

        let reply = match db_type {
            DbType::Shared(Shared { engine }) => {
                self.request_shared_db(&request.project_name, engine.expect("oneof to be set"))
                    .await?
            }
            DbType::AwsRds(AwsRds { engine }) => {
                self.request_aws_rds(&request.project_name, engine.expect("oneof to be set"))
                    .await?
            }
            DbType::ElastiCache(ElastiCache { engine }) => {
                self.request_elasticache(&request.project_name, engine.expect("oneof to be set"))
                    .await?
            }
        };

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
            .get(0)
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

async fn wait_for_cluster(
    client: &aws_sdk_elasticache::Client,
    name: &str,
    wait_for: &str,
) -> Result<CacheCluster, Error> {
    debug!("waiting for {name} to enter {wait_for} state");
    loop {
        let cluster = client
            .describe_cache_clusters()
            .cache_cluster_id(name)
            .send()
            .await?
            .cache_clusters()
            .expect("aws to return clusters")
            .get(0)
            .expect("to find the cluster just created or modified")
            .clone();

        let status = cluster
            .cache_cluster_status()
            .as_ref()
            .expect("instance to have a status")
            .clone();

        if status == wait_for {
            return Ok(cluster);
        }

        sleep(Duration::from_secs(1)).await;
    }
}

fn engine_to_port(engine: aws_rds::Engine) -> String {
    match engine {
        aws_rds::Engine::Postgres(_) => "5432".to_string(),
        aws_rds::Engine::Mariadb(_) => "3306".to_string(),
        aws_rds::Engine::Mysql(_) => "3306".to_string(),
    }
}
