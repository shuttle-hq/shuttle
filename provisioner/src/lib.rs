use std::time::Duration;

pub use args::Args;
use aws_config::timeout;
use aws_sdk_iam;
use aws_sdk_iam::operation::create_policy::CreatePolicyError;
use aws_sdk_iam::operation::create_user::CreateUserError;
use aws_sdk_iam::operation::delete_user::DeleteUserOutput;
use aws_sdk_rds::{
    error::SdkError, operation::modify_db_instance::ModifyDBInstanceError, types::DbInstance,
    Client,
};
use aws_sdk_sts;
pub use error::Error;
use mongodb::{bson::doc, options::ClientOptions};
use rand::Rng;
use serde_json::json;
use shuttle_common::claims::{Claim, Scope};
use shuttle_proto::provisioner::{DynamoDbResponse, DynamoDbRequest, DynamoDbDeletionResponse};
pub use shuttle_proto::provisioner::provisioner_server::ProvisionerServer;
use shuttle_proto::provisioner::{
    aws_rds, database_request::DbType, shared, AwsRds, DatabaseRequest, DatabaseResponse, Shared,
};
use shuttle_proto::provisioner::{provisioner_server::Provisioner, DatabaseDeletionResponse};
use sqlx::{postgres::PgPoolOptions, ConnectOptions, Executor, PgPool};
use tokio::time::sleep;
use tonic::{Request, Response, Status};
use tracing::{debug, info};
use std::fs::File;
use std::io::BufRead;

mod args;
mod error;

const AWS_RDS_CLASS: &str = "db.t4g.micro";
const MASTER_USERNAME: &str = "master";
const RDS_SUBNET_GROUP: &str = "shuttle_rds";

pub struct MyProvisioner {
    pool: PgPool,
    rds_client: aws_sdk_rds::Client,
    mongodb_client: mongodb::Client,
    dynamodb_client: aws_sdk_dynamodb::Client,
    iam_client: aws_sdk_iam::Client,
    sts_client: aws_sdk_sts::Client,
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

        let dynamodb_client = aws_sdk_dynamodb::Client::new(&aws_config);

        let iam_client = aws_sdk_iam::Client::new(&aws_config);

        let sts_client = aws_sdk_sts::Client::new(&aws_config);

        Ok(Self {
            pool,
            rds_client,
            mongodb_client,
            dynamodb_client,
            iam_client,
            sts_client,
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

            // Make sure database can't see other databases or other users
            // For #557
            let options = self.pool.connect_options().clone().database(&database_name);
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

    async fn get_prefix(&self, project_name: &str) -> String {
        //TODO: add userid or something else unique here
        format!("shuttle-dynamodb-{}-", project_name)
    }

    async fn get_dynamodb_policy_name(&self, prefix: &str) -> String {
        format!("{}policy", prefix)
    }

    async fn create_dynamodb_policy(&self, prefix: &str) -> Result<(), Error> {
        let table_name = format!("arn:aws:dynamodb:*:*:table/{}*", prefix);
        let policy_document = json!({
            "Version": "2012-10-17",
            "Statement": [
                {
                    "Sid": "SpecificTable",
                    "Effect": "Allow",
                    "Action": [
                        "dynamodb:BatchGet*",
                        "dynamodb:DescribeStream",
                        "dynamodb:DescribeTable",
                        "dynamodb:Get*",
                        "dynamodb:Query",
                        "dynamodb:Scan",
                        "dynamodb:BatchWrite*",
                        "dynamodb:CreateTable",
                        "dynamodb:Delete*",
                        "dynamodb:Update*",
                        "dynamodb:PutItem",
                        "dynamodb:List*",
                        "dynamodb:DescribeReservedCapacity*",
                        "dynamodb:DescribeLimits",
                        "dynamodb:DescribeTimeToLive"
                    ],
                    "Resource": table_name
                }
            ]
        })
        .to_string();

        let policy_name = self.get_dynamodb_policy_name(prefix).await;

        match self
            .iam_client
            .create_policy()
            .policy_name(policy_name)
            .policy_document(policy_document)
            .send()
            .await
        {
            Ok(_) => {}
            Err(e) => {
                match e.into_service_error() {
                    CreatePolicyError::EntityAlreadyExistsException(_) => {} // for idempotency
                    e => {
                        return Err(Error::CreateIAMPolicy(e));
                    }
                }
            }
        };

        Ok(())
    }

    async fn get_policy_arn(&self, prefix: &str) -> Result<String, Error> {
        let identity = self.sts_client.get_caller_identity().send().await.unwrap();
        let account = identity.account().unwrap();

        let policy_name = self.get_dynamodb_policy_name(prefix).await;
        let policy_arn = format!("arn:aws:iam::{account}:policy/{policy_name}");

        Ok(policy_arn)
    }

    async fn delete_dynamodb_policy(&self, prefix: &str) -> Result<(), Error> {
        let policy_arn = self.get_policy_arn(&prefix).await.unwrap();

        self.iam_client
            .delete_policy()
            .policy_arn(policy_arn)
            .send()
            .await
            .unwrap();

        Ok(())
    }

    async fn get_iam_identity_user_name(&self, prefix: &str) -> String {
        format!("{}user", prefix)
    }

    async fn create_iam_identity(&self, prefix: &str) -> Result<(), Error> {
        match self
            .iam_client
            .create_user()
            .user_name(self.get_iam_identity_user_name(prefix).await)
            .send()
            .await {
                Ok(_) => {},
                Err(e) => {
                    match e.into_service_error() {
                        CreateUserError::EntityAlreadyExistsException(_) => {}
                        e => {
                            return Err(Error::CreateIAMUser(e));
                        }
                    }
                }
            };
        Ok(())
    }

    async fn get_saved_access_key(&self, prefix: &str) -> Option<(String, String)> {
        if let Ok(file) = std::fs::File::open(self.get_access_key_file_name(prefix)) {
            let mut lines = std::io::BufReader::new(file).lines();

            if let Some(Ok(access_key_id)) = lines.next() {
                if let Some(Ok(secret_access_key)) = lines.next() {
                    return Some((access_key_id, secret_access_key))
                }
            }
        }

        None
    }

    fn get_access_key_file_name(&self, prefix: &str) -> String {
        format!("{}.txt", prefix)
    }

    async fn delete_saved_access_key(&self, prefix: &str) -> Result<(), std::io::Error> {
        std::fs::remove_file(self.get_access_key_file_name(prefix))?;
        Ok(())
    }

    async fn save_access_key(&self, prefix: &str, access_key_id: &str, secret_access_key: &str) -> Result<(), std::io::Error> {
        use std::io::prelude::*;    
        let mut file = File::create(self.get_access_key_file_name(prefix))?;
        let contents = format!("{}\n{}", access_key_id, secret_access_key);
        file.write_all(contents.as_bytes())?;

        Ok(())
    }

    async fn get_iam_identity_keys(&self, prefix: &str) -> Result<(String, String), Error> {
        if let Some((access_key_id, secret_access_key)) = self.get_saved_access_key(prefix).await {
            return Ok((access_key_id, secret_access_key));
        }

        let key = self
            .iam_client
            .create_access_key()
            .user_name(self.get_iam_identity_user_name(prefix).await)
            .send()
            .await
            .unwrap();
        let access_key = key
            .access_key()
            .unwrap();

        let access_key_id = access_key.access_key_id.as_ref().unwrap().to_string();
        let secret_access_key = access_key.secret_access_key.as_ref().unwrap().to_string();

        self.save_access_key(prefix, &access_key_id, &secret_access_key).await.map_err(|e| Error::GetIAMIdentityKeys(e))?;

        Ok((access_key_id, secret_access_key))
    }

    async fn delete_iam_identity(&self, prefix: &str) -> Result<DeleteUserOutput, Error> {
        let user = self
            .iam_client
            .delete_user()
            .user_name(self.get_iam_identity_user_name(prefix).await)
            .send()
            .await
            .unwrap();
        Ok(user)
    }

    async fn attach_user_policy(&self, prefix: &str) -> Result<(), Error> {
        self
            .iam_client
            .attach_user_policy()
            .user_name(self.get_iam_identity_user_name(&prefix).await)
            .policy_arn(self.get_policy_arn(&prefix).await.unwrap())
            .send()
            .await
            .unwrap();
        Ok(())
    }

    async fn detach_user_policy(&self, prefix: &str) -> Result<(), Error> {
        self
            .iam_client
            .detach_user_policy()
            .user_name(self.get_iam_identity_user_name(&prefix).await)
            .policy_arn(self.get_policy_arn(&prefix).await.unwrap())
            .send()
            .await
            .unwrap();
        Ok(())
    }

    pub async fn request_dynamodb(&self, project_name: &str) -> Result<DynamoDbResponse, Error> {
        let prefix = self.get_prefix(&project_name).await;

        self.create_dynamodb_policy(&prefix).await.unwrap();

        self.create_iam_identity(&prefix).await.unwrap();

        self.attach_user_policy(&prefix).await.unwrap();

        let (aws_access_key_id, aws_secret_access_key)= self.get_iam_identity_keys(&prefix).await.unwrap();

        let aws_default_region = self.dynamodb_client.conf().region().unwrap().to_string();

        //TODO:
        //store aws credentials in secrets
        //make them available in the project container
        //setup dynamodb client

        //create new example app with annotation, creating a table and then inserting a row

        //as part of our delete, wipe out any tables with the matching prefix, we delete the policy, we delete the identity

        //NOTE: for future, maybe allow multiple projects to access same database (for creating, just use username as prefix, for deleting will need to query whether projects exist)

        // let client = &self.dynamodb_client;
        Ok(DynamoDbResponse {
            prefix,
            aws_access_key_id,
            aws_secret_access_key,
            aws_default_region,
            endpoint: None
        })
    }

    async fn delete_dynamodb_tables_by_prefix(&self, prefix: &str) {
        let mut last_evaluated_table_name: Option<String> = Some(prefix.to_string());

        'outer: while last_evaluated_table_name.is_some() {
            let result = self.dynamodb_client.list_tables().exclusive_start_table_name(last_evaluated_table_name.unwrap()).send().await.unwrap();
            last_evaluated_table_name = result.last_evaluated_table_name.clone();

            if let Some(table_names) = result.table_names {
                for table_name in table_names {
                    if !table_name.starts_with(&prefix) {
                        break 'outer;
                    } else {
                        self.dynamodb_client.delete_table().table_name(table_name).send().await.unwrap();
                    }
                }
            }
        }

        // edge case to include just the prefix table name (if the user put only prefix for table name)
        let _ = self.dynamodb_client.delete_table().table_name(prefix).send().await;
    }

    async fn delete_dynamodb(&self, project_name: &str) -> Result<DynamoDbDeletionResponse, Error> {
        let prefix = self.get_prefix(project_name).await;
        self.detach_user_policy(&prefix).await?;
        self.delete_iam_identity(&prefix).await?;
        self.delete_dynamodb_policy(&prefix).await?;

        self.delete_dynamodb_tables_by_prefix(&prefix).await;


        self.delete_saved_access_key(&prefix).await?;


        Ok(DynamoDbDeletionResponse {})
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
                if let ModifyDBInstanceError::DbInstanceNotFoundFault(_) = err.err() {
                    debug!("creating new AWS RDS {instance_name}");

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

    async fn delete_shared_db(
        &self,
        project_name: &str,
        engine: shared::Engine,
    ) -> Result<DatabaseDeletionResponse, Error> {
        match engine {
            shared::Engine::Postgres(_) => self.delete_pg(project_name).await?,
            shared::Engine::Mongodb(_) => self.delete_mongodb(project_name).await?,
        }
        Ok(DatabaseDeletionResponse {})
    }

    async fn delete_pg(&self, project_name: &str) -> Result<(), Error> {
        let database_name = format!("db-{project_name}");
        let role_name = format!("user-{project_name}");

        // Idenfitiers cannot be used as query parameters
        let drop_db_query = format!("DROP DATABASE \"{database_name}\";");

        // Drop the database. Note that this can fail if there are still active connections to it
        sqlx::query(&drop_db_query)
            .execute(&self.pool)
            .await
            .map_err(|e| Error::DeleteRole(e.to_string()))?;

        // Drop the role
        let drop_role_query = format!("DROP ROLE IF EXISTS \"{role_name}\"");
        sqlx::query(&drop_role_query)
            .execute(&self.pool)
            .await
            .map_err(|e| Error::DeleteDB(e.to_string()))?;

        Ok(())
    }

    async fn delete_mongodb(&self, project_name: &str) -> Result<(), Error> {
        let database_name = format!("mongodb-{project_name}");
        let db = self.mongodb_client.database(&database_name);

        // dropping a database in mongodb doesn't delete any associated users
        // so do that first

        let drop_users_command = doc! {
            "dropAllUsersFromDatabase": 1
        };

        db.run_command(drop_users_command, None)
            .await
            .map_err(|e| Error::DeleteRole(e.to_string()))?;

        // drop the actual database

        db.drop(None)
            .await
            .map_err(|e| Error::DeleteDB(e.to_string()))?;

        Ok(())
    }

    async fn delete_aws_rds(
        &self,
        project_name: &str,
        engine: aws_rds::Engine,
    ) -> Result<DatabaseDeletionResponse, Error> {
        let client = &self.rds_client;
        let instance_name = format!("{project_name}-{engine}");

        // try to delete the db instance
        let delete_result = client
            .delete_db_instance()
            .db_instance_identifier(&instance_name)
            .send()
            .await;

        // Did we get an error that wasn't "db instance not found"
        if let Err(SdkError::ServiceError(err)) = delete_result {
            if !err.err().is_db_instance_not_found_fault() {
                return Err(Error::Plain(format!(
                    "got unexpected error from AWS RDS service: {}",
                    err.err()
                )));
            }
        }

        Ok(DatabaseDeletionResponse {})
    }
}

#[tonic::async_trait]
impl Provisioner for MyProvisioner {
    #[tracing::instrument(skip(self))]
    async fn provision_database(
        &self,
        request: Request<DatabaseRequest>,
    ) -> Result<Response<DatabaseResponse>, Status> {
        verify_claim(&request)?;

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
        };

        Ok(Response::new(reply))
    }

    #[tracing::instrument(skip(self))]
    async fn delete_database(
        &self,
        request: Request<DatabaseRequest>,
    ) -> Result<Response<DatabaseDeletionResponse>, Status> {
        verify_claim(&request)?;

        let request = request.into_inner();
        let db_type = request.db_type.unwrap();

        let reply = match db_type {
            DbType::Shared(Shared { engine }) => {
                self.delete_shared_db(&request.project_name, engine.expect("oneof to be set"))
                    .await?
            }
            DbType::AwsRds(AwsRds { engine }) => {
                self.delete_aws_rds(&request.project_name, engine.expect("oneof to be set"))
                    .await?
            }
        };

        Ok(Response::new(reply))
    }

    #[tracing::instrument(skip(self))]
    async fn provision_dynamo_db(
        &self,
        request: Request<DynamoDbRequest>,
    ) -> Result<Response<DynamoDbResponse>, Status> {
        verify_claim(&request)?;

        let request = request.into_inner();

        let reply = self.request_dynamodb(&request.project_name).await?;

        Ok(Response::new(reply))
    }

    #[tracing::instrument(skip(self))]
    async fn delete_dynamo_db(
        &self,
        request: Request<DynamoDbRequest>,
    ) -> Result<Response<DynamoDbDeletionResponse>, Status> {
        verify_claim(&request)?;

        let request = request.into_inner();

        let reply = self.delete_dynamodb(&request.project_name).await?;

        Ok(Response::new(reply))
    }
}

/// Verify the claim on the request has the correct scope to call this service
fn verify_claim<B>(request: &Request<B>) -> Result<(), Status> {
    let claim = request
        .extensions()
        .get::<Claim>()
        .ok_or_else(|| Status::internal("could not get claim"))?;

    if claim.scopes.contains(&Scope::ResourcesWrite) {
        Ok(())
    } else {
        Err(Status::permission_denied(
            "does not have resource allocation scope",
        ))
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

fn engine_to_port(engine: aws_rds::Engine) -> String {
    match engine {
        aws_rds::Engine::Postgres(_) => "5432".to_string(),
        aws_rds::Engine::Mariadb(_) => "3306".to_string(),
        aws_rds::Engine::Mysql(_) => "3306".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use aws_sdk_dynamodb::types::{AttributeDefinition, ScalarAttributeType, KeySchemaElement, KeyType, ProvisionedThroughput};
    use tokio::time::sleep;

    use crate::MyProvisioner;

    async fn make_test_provisioner() -> MyProvisioner {
        let pg_uri = format!("postgres://postgres:password@localhost:5432");
        let mongo_uri = format!("mongodb://mongodb:password@localhost:8080");

        let provisioner = MyProvisioner::new(
            &pg_uri,
            &mongo_uri,
            "fqdn".to_string(),
            "pg".to_string(),
            "mongodb".to_string(),
        )
        .await
        .unwrap();

        provisioner
    }

    async fn create_dynamodb_table(dynamodb_client: &aws_sdk_dynamodb::Client, table_name: &str) {
        let attribute_definition = AttributeDefinition::builder()
        .attribute_name("test")
        .attribute_type(ScalarAttributeType::S)
        .build();

        let key_schema = KeySchemaElement::builder()
            .attribute_name("test")
            .key_type(KeyType::Hash)
            .build();

        let provisioned_throughput = ProvisionedThroughput::builder()
            .read_capacity_units(10)
            .write_capacity_units(5)
            .build();

        dynamodb_client
        .create_table()
        .table_name(table_name)
        .key_schema(key_schema.clone())
        .attribute_definitions(attribute_definition.clone())
        .provisioned_throughput(provisioned_throughput.clone())
        .send()
        .await.unwrap();
    }


    #[tokio::test]
    async fn test_create_and_delete_dynamodb_policy() {
        let provisioner = make_test_provisioner().await;

        let prefix = provisioner.get_prefix("my_cool_project").await;

        let result = provisioner.create_dynamodb_policy(&prefix).await.unwrap();

        println!("{result:?}");

        provisioner.delete_dynamodb_policy(&prefix).await.unwrap();
    }

    #[tokio::test]
    async fn test_create_and_delete_aws_user() {
        let provisioner = make_test_provisioner().await;

        let prefix = provisioner.get_prefix("my_cool_project").await;

        provisioner.create_iam_identity(&prefix).await.unwrap();

        provisioner.delete_iam_identity(&prefix).await.unwrap();
    }
     
    #[tokio::test]
    async fn test_request_dynamodb_multiple_times() {
        let provisioner = make_test_provisioner().await;

        provisioner
        .request_dynamodb("test_request_dynamodb") //NOTE: User names should be less that 64 characters
        .await
        .unwrap();

        // you should be able to request the same resource multiple times without error
        provisioner
        .request_dynamodb("test_request_dynamodb")
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn test_delete_dynamodb() {
        let provisioner = make_test_provisioner().await;

        provisioner
        .request_dynamodb("test_delete_dynamodb")
        .await
        .unwrap();

        provisioner
        .delete_dynamodb("test_delete_dynamodb")
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn test_dynamodb_delete_table_names_by_prefix() {
        let provisioner = make_test_provisioner().await;

        let prefix = "test_dynamodb_delete_table_names_by_prefix-";

        create_dynamodb_table(&provisioner.dynamodb_client, &format!("{}1", prefix)).await;
        create_dynamodb_table(&provisioner.dynamodb_client, &format!("{}2", prefix)).await;
        create_dynamodb_table(&provisioner.dynamodb_client, &format!("{}", prefix)).await;

        //takes a while for dynamodb tables to provision
        sleep(Duration::from_secs(10)).await;

        provisioner.delete_dynamodb_tables_by_prefix(prefix).await;
    }

    #[tokio::test]
    async fn test_get_access_key() {
        let provisioner = make_test_provisioner().await;

        let access_key_id = "my-access-key".to_string();
        let secret_access_key = "my-secret-access-key".to_string();
        let prefix = provisioner.get_prefix("test_get_access_key").await;

        assert_eq!(provisioner.get_saved_access_key(&prefix).await, None);

        provisioner.save_access_key(&prefix, &access_key_id, &secret_access_key).await.unwrap();

        assert_eq!(provisioner.get_saved_access_key(&prefix).await, Some((access_key_id, secret_access_key)));

        provisioner.delete_saved_access_key(&prefix).await.unwrap();        
    }
}
