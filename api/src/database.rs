use anyhow::anyhow;
use aws_config::{
    environment::EnvironmentVariableCredentialsProvider,
    imds,
    meta::{credentials::CredentialsProviderChain, region::RegionProviderChain},
    timeout,
};
use aws_sdk_rds::{error::ModifyDBInstanceErrorKind, types::SdkError};
use aws_smithy_types::tristate::TriState;
use lazy_static::lazy_static;
use rand::Rng;
use shuttle_common::{project::ProjectName, DatabaseReadyInfo};
use shuttle_service::{database::AwsRdsEngine, error::CustomError};
use sqlx::postgres::{PgPool, PgPoolOptions};
use std::time::Duration;
use tokio::time::sleep;

#[cfg(debug_assertions)]
const PUBLIC_PG_IP: &str = "localhost";

#[cfg(not(debug_assertions))]
const PUBLIC_PG_IP: &'static str = "pg.shuttle.rs";

const AWS_RDS_CLASS: &str = "db.t4g.micro";
const MASTER_USERNAME: &str = "master";

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
    project: ProjectName,
    context: Context,
    info: Option<DatabaseReadyInfo>,
}

impl State {
    pub(crate) fn new(project: &ProjectName, context: &Context) -> Self {
        Self {
            project: project.clone(),
            context: context.clone(),
            info: None,
        }
    }

    pub(crate) async fn request(&mut self) -> sqlx::Result<DatabaseReadyInfo> {
        if self.info.is_some() {
            // Safe to unwrap since we just confirmed it is `Some`
            return Ok(self.info.clone().unwrap());
        }

        let role_name = format!("user-{}", self.project);
        let role_password = generate_role_password();
        let database_name = format!("db-{}", self.project);

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
            sqlx::query(&create_role_query).execute(pool).await?;

            debug!(
                "created new role '{}' in database for project '{}'",
                role_name, database_name
            );
        } else {
            // If the role already exists then change its password:
            let alter_password_query = format!(
                "ALTER ROLE \"{}\" WITH PASSWORD '{}'",
                role_name, role_password
            );
            sqlx::query(&alter_password_query).execute(pool).await?;

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
            sqlx::query(&create_database_query).execute(pool).await?;

            debug!(
                "created database '{}' belonging to '{}'",
                database_name, role_name
            );
        } else {
            debug!(
                "database '{}' already exists, not recreating",
                database_name
            );
        }

        let info = DatabaseReadyInfo::new(
            "postgres".to_string(),
            role_name,
            role_password,
            database_name,
            "localhost".to_string(),
            PUBLIC_PG_IP.to_string(),
        );
        self.info = Some(info.clone());
        Ok(info)
    }

    pub(crate) fn to_info(&self) -> Option<DatabaseReadyInfo> {
        self.info.clone()
    }

    pub(crate) async fn aws_rds(
        &mut self,
        engine: AwsRdsEngine,
    ) -> Result<DatabaseReadyInfo, shuttle_service::Error> {
        println!("getting rds");
        if self.info.is_some() {
            // Safe to unwrap since we just confirmed it is `Some`
            return Ok(self.info.clone().unwrap());
        }

        println!("getting client");
        error!("getting client");
        let client = &self.context.rds_client;

        let password = generate_role_password();
        let instance_name = format!("{}-{}", self.project, engine);

        println!("getting modified instance");
        let instance = client
            .modify_db_instance()
            .db_instance_identifier(&instance_name)
            .master_user_password(&password)
            .send()
            .await;

        println!("checking status");
        let mut instance = match instance {
            Ok(instance) => instance
                .db_instance
                .expect("aws response should contain an instance")
                .clone(),
            Err(SdkError::ServiceError { err, .. }) => {
                if let ModifyDBInstanceErrorKind::DbInstanceNotFoundFault(_) = err.kind {
                    debug!("creating new AWS RDS for {}", self.project);

                    client
                        .create_db_instance()
                        .db_instance_identifier(&instance_name)
                        .master_username(MASTER_USERNAME)
                        .master_user_password(&password)
                        .engine(engine.to_string())
                        .db_instance_class(AWS_RDS_CLASS)
                        .allocated_storage(20)
                        .backup_retention_period(0)
                        .publicly_accessible(true)
                        .db_name(engine.to_string())
                        .send()
                        .await
                        .map_err(shuttle_service::error::CustomError::new)?
                        .db_instance
                        .expect("to be able to create instance")
                } else {
                    return Err(shuttle_service::Error::Custom(anyhow!(
                        "got unexpected error from AWS RDS service: {}",
                        err
                    )));
                }
            }
            Err(unexpected) => {
                return Err(shuttle_service::Error::Custom(anyhow!(
                    "got unexpected error from AWS during API call: {}",
                    unexpected
                )))
            }
        };

        // Wait for up
        debug!("waiting for password update");
        sleep(Duration::from_secs(30)).await;
        loop {
            instance = client
                .describe_db_instances()
                .db_instance_identifier(&instance_name)
                .send()
                .await
                .map_err(CustomError::new)?
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

            if status == "available" {
                break;
            }

            sleep(Duration::from_secs(1)).await;
        }

        let address = instance
            .endpoint
            .expect("instance to have an endpoint")
            .address
            .expect("endpoint to have an address");

        let info = DatabaseReadyInfo::new(
            engine.to_string(),
            instance
                .master_username
                .expect("instance to have a username"),
            password,
            instance
                .db_name
                .expect("instance to have a default database"),
            address.clone(),
            address,
        );

        self.info = Some(info.clone());

        Ok(info)
    }
}

#[derive(Clone)]
pub struct Context {
    sudo_pool: PgPool,
    rds_client: aws_sdk_rds::Client,
}

impl Context {
    pub async fn new() -> sqlx::Result<Self> {
        let sudo_pool = PgPoolOptions::new()
            .min_connections(4)
            .max_connections(12)
            .connect_timeout(Duration::from_secs(60))
            .connect_lazy(&SUDO_POSTGRES_CONNECTION_STRING)?;

        let api_timeout_config = timeout::Api::new()
            .with_call_timeout(TriState::Set(Duration::from_secs(120)))
            .with_call_attempt_timeout(TriState::Set(Duration::from_secs(120)));
        let timeout_config = timeout::Config::new().with_api_timeouts(api_timeout_config);
        let region_provider = RegionProviderChain::default_provider().or_else("eu-west-2");

        let env_provider = EnvironmentVariableCredentialsProvider::new();
        let imds_provider = imds::credentials::Builder::default()
            .profile("BackendAPIRole")
            .build();

        let chained_provider = CredentialsProviderChain::first_try("Environment", env_provider)
            .or_else("Ec2InstanceMetadata", imds_provider);

        let aws_config = aws_config::from_env()
            .timeout_config(timeout_config)
            .region(region_provider)
            .credentials_provider(chained_provider)
            .load()
            .await;

        let rds_client = aws_sdk_rds::Client::new(&aws_config);

        Ok(Self {
            sudo_pool,
            rds_client,
        })
    }
}
