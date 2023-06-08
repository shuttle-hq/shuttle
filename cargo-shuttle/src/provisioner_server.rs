use anyhow::Result;
use async_trait::async_trait;
use bollard::{
    container::{Config, CreateContainerOptions, StartContainerOptions},
    exec::{CreateExecOptions, CreateExecResults},
    image::CreateImageOptions,
    models::{CreateImageInfo, HostConfig, PortBinding, ProgressDetail},
    service::ContainerInspectResponse,
    Docker,
};
use crossterm::{
    cursor::{MoveDown, MoveUp},
    terminal::{Clear, ClearType},
    QueueableCommand,
};
use futures::StreamExt;
use portpicker::pick_unused_port;
use shuttle_common::{
    database::{AwsRdsEngine, SharedEngine},
    delete_dynamodb_tables_by_prefix, DynamoDbReadyInfo,
};
use shuttle_proto::provisioner::{
    provisioner_server::{Provisioner, ProvisionerServer},
    DatabaseDeletionResponse, DatabaseRequest, DatabaseResponse, DynamoDbDeletionResponse,
    DynamoDbRequest, DynamoDbResponse,
};
use shuttle_service::database::Type;
use std::{collections::HashMap, io::stdout, net::SocketAddr, time::Duration};
use tokio::{task::JoinHandle, time::sleep};
use tonic::{
    transport::{self, Server},
    Request, Response, Status,
};
use tracing::{error, trace};

/// A provisioner for local runs
/// It uses Docker to create Databases
pub struct LocalProvisioner {
    docker: Docker,
}

impl LocalProvisioner {
    pub fn new() -> Result<Self> {
        Ok(Self {
            docker: Docker::connect_with_local_defaults()?,
        })
    }

    pub fn start(self, address: SocketAddr) -> JoinHandle<Result<(), transport::Error>> {
        tokio::spawn(async move {
            Server::builder()
                .add_service(ProvisionerServer::new(self))
                .serve(address)
                .await
        })
    }

    async fn get_prefix(&self, project_name: &str) -> String {
        format!("shuttle-dynamodb-{}-", project_name)
    }

    fn get_container_host_port(&self, container: ContainerInspectResponse, port: &str) -> String {
        container
            .host_config
            .expect("container to have host config")
            .port_bindings
            .expect("port bindings on container")
            .get(port)
            .expect("a port bindings entry")
            .as_ref()
            .expect("a port bindings")
            .first()
            .expect("at least one port binding")
            .host_port
            .as_ref()
            .expect("a host port")
            .clone()
    }

    async fn start_container_if_not_running(
        &self,
        container: ContainerInspectResponse,
        container_type: &str,
        container_name: &str,
    ) {
        if !container
            .state
            .expect("container to have a state")
            .running
            .expect("state to have a running key")
        {
            trace!("{container_type} container '{container_name}' not running, so starting it");
            self.docker
                .start_container(container_name, None::<StartContainerOptions<String>>)
                .await
                .expect("failed to start none running container");
        }
    }

    async fn get_container(
        &self,
        container_name: &str,
        image: String,
        port: &str,
        env: Option<Vec<String>>,
    ) -> Result<ContainerInspectResponse, Status> {
        match self.docker.inspect_container(container_name, None).await {
            Ok(container) => {
                trace!("found DB container {container_name}");
                Ok(container)
            }
            Err(bollard::errors::Error::DockerResponseServerError { status_code, .. })
                if status_code == 404 =>
            {
                self.pull_image(&image).await.expect("failed to pull image");
                trace!("will create DB container {container_name}");
                let options = Some(CreateContainerOptions {
                    name: container_name.clone(),
                    platform: None,
                });
                let mut port_bindings = HashMap::new();
                let host_port = pick_unused_port().expect("system to have a free port");
                port_bindings.insert(
                    port.to_string(),
                    Some(vec![PortBinding {
                        host_port: Some(host_port.to_string()),
                        ..Default::default()
                    }]),
                );
                let host_config = HostConfig {
                    port_bindings: Some(port_bindings),
                    ..Default::default()
                };

                let config: Config<String> = Config {
                    image: Some(image.to_string()),
                    env,
                    host_config: Some(host_config),
                    ..Default::default()
                };

                self.docker
                    .create_container(options, config)
                    .await
                    .expect("to be able to create container");

                Ok(self
                    .docker
                    .inspect_container(container_name, None)
                    .await
                    .expect("container to be created"))
            }
            Err(error) => {
                error!("got unexpected error while inspecting docker container: {error}");
                Err(Status::internal(error.to_string()))
            }
        }
    }

    async fn get_dynamodb_connection_info(
        &self,
        project_name: &str,
    ) -> Result<DynamoDbReadyInfo, Status> {
        let DynamoDbConfig {
            container_name,
            image,
            port,
            aws_access_key_id,
            aws_secret_access_key,
            aws_default_region,
        } = dynamodb_config();

        let env = None;

        let container = self
            .get_container(&container_name, image, &port, env)
            .await?;

        let port = self.get_container_host_port(container.clone(), &port);

        self.start_container_if_not_running(container, "DynamoDB", &container_name)
            .await;

        let endpoint = format!("http://localhost:{}", port);

        //https://docs.aws.amazon.com/amazondynamodb/latest/developerguide/DynamoDBLocal.DownloadingAndRunning.html#docker
        Ok(DynamoDbReadyInfo {
            prefix: self.get_prefix(project_name).await,
            aws_access_key_id,
            aws_secret_access_key,
            aws_default_region,
            endpoint: Some(endpoint),
        })
    }

    async fn get_db_connection_string(
        &self,
        service_name: &str,
        db_type: Type,
    ) -> Result<DatabaseResponse, Status> {
        trace!("getting sql string for service '{}'", service_name);

        let EngineConfig {
            r#type,
            image,
            engine,
            username,
            password,
            database_name,
            port,
            env,
            is_ready_cmd,
        } = db_type_to_config(db_type);
        let container_name = format!("shuttle_{service_name}_{type}");

        let container = self
            .get_container(&container_name, image, &port, env)
            .await?;

        let port = self.get_container_host_port(container.clone(), &port);

        self.start_container_if_not_running(container, "DB", &container_name)
            .await;

        self.wait_for_ready(&container_name, is_ready_cmd).await?;

        let res = DatabaseResponse {
            engine,
            username,
            password,
            database_name,
            port,
            address_private: "localhost".to_string(),
            address_public: "localhost".to_string(),
        };

        Ok(res)
    }

    async fn wait_for_ready(
        &self,
        container_name: &str,
        is_ready_cmd: Vec<String>,
    ) -> Result<(), Status> {
        loop {
            trace!("waiting for '{container_name}' to be ready for connections");

            let config = CreateExecOptions {
                cmd: Some(is_ready_cmd.clone()),
                attach_stdout: Some(true),
                attach_stderr: Some(true),
                ..Default::default()
            };

            let CreateExecResults { id } = self
                .docker
                .create_exec(container_name, config)
                .await
                .expect("failed to create exec to check if container is ready");

            let ready_result = self
                .docker
                .start_exec(&id, None)
                .await
                .expect("failed to execute ready command");

            if let bollard::exec::StartExecResults::Attached { mut output, .. } = ready_result {
                while let Some(line) = output.next().await {
                    trace!("line: {:?}", line);

                    if let bollard::container::LogOutput::StdOut { .. } =
                        line.expect("output to have a log line")
                    {
                        return Ok(());
                    }
                }
            }

            sleep(Duration::from_millis(500)).await;
        }
    }

    async fn delete_dynamodb_tables_by_prefix_in_container(
        &self,
        prefix: &str,
    ) -> Result<DynamoDbDeletionResponse, Status> {
        let DynamoDbConfig {
            container_name,
            image: _,
            port,
            aws_access_key_id,
            aws_secret_access_key,
            aws_default_region,
        } = dynamodb_config();

        let container = match self.docker.inspect_container(&container_name, None).await {
            Ok(container) => {
                trace!("found DB container {container_name}");
                container
            }
            Err(error) => {
                error!("got unexpected error while inspecting docker container: {error}");
                return Err(Status::internal(error.to_string()));
            }
        };

        let port = self.get_container_host_port(container.clone(), &port);

        std::env::set_var("AWS_ACCESS_KEY_ID", aws_access_key_id);
        std::env::set_var("AWS_SECRET_ACCESS_KEY", aws_secret_access_key);
        std::env::set_var("AWS_REGION", aws_default_region);

        let endpoint = format!("http://localhost:{}", port);

        let aws_config = aws_config::from_env().endpoint_url(endpoint).load().await;

        let dynamodb_client = aws_sdk_dynamodb::Client::new(&aws_config);

        delete_dynamodb_tables_by_prefix(&dynamodb_client, prefix)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(DynamoDbDeletionResponse {})
    }

    async fn pull_image(&self, image: &str) -> Result<(), String> {
        trace!("pulling latest image for '{image}'");
        let mut layers = Vec::new();

        let create_image_options = Some(CreateImageOptions {
            from_image: image,
            ..Default::default()
        });
        let mut output = self.docker.create_image(create_image_options, None, None);

        while let Some(line) = output.next().await {
            let info = line.expect("failed to create image");

            if let Some(id) = info.id.as_ref() {
                match layers
                    .iter_mut()
                    .find(|item: &&mut CreateImageInfo| item.id.as_deref() == Some(id))
                {
                    Some(item) => *item = info,
                    None => layers.push(info),
                }
            } else {
                layers.push(info);
            }

            print_layers(&layers);
        }

        // Undo last MoveUps
        stdout()
            .queue(MoveDown(
                layers.len().try_into().expect("to convert usize to u16"),
            ))
            .expect("to reset cursor position");

        Ok(())
    }
}

#[async_trait]
impl Provisioner for LocalProvisioner {
    async fn provision_database(
        &self,
        request: Request<DatabaseRequest>,
    ) -> Result<Response<DatabaseResponse>, Status> {
        let DatabaseRequest {
            project_name,
            db_type,
        } = request.into_inner();

        let db_type: Option<Type> = db_type.unwrap().into();

        let res = self
            .get_db_connection_string(&project_name, db_type.unwrap())
            .await?;

        Ok(Response::new(res))
    }

    async fn delete_database(
        &self,
        _request: Request<DatabaseRequest>,
    ) -> Result<Response<DatabaseDeletionResponse>, Status> {
        panic!("local runner should not try to delete databases");
    }

    async fn provision_dynamo_db(
        &self,
        request: Request<DynamoDbRequest>,
    ) -> Result<Response<DynamoDbResponse>, Status> {
        let DynamoDbRequest { project_name } = request.into_inner();

        let res = self.get_dynamodb_connection_info(&project_name).await?;

        Ok(Response::new(res.into()))
    }

    async fn delete_dynamo_db(
        &self,
        request: Request<DynamoDbRequest>,
    ) -> Result<Response<DynamoDbDeletionResponse>, Status> {
        let DynamoDbRequest { project_name } = request.into_inner();

        let res = self
            .delete_dynamodb_tables_by_prefix_in_container(&project_name)
            .await?;

        Ok(Response::new(res))
    }
}

fn print_layers(layers: &Vec<CreateImageInfo>) {
    for info in layers {
        stdout()
            .queue(Clear(ClearType::CurrentLine))
            .expect("to be able to clear line");

        if let Some(id) = info.id.as_ref() {
            let text = match (info.status.as_deref(), info.progress_detail.as_ref()) {
                (
                    Some("Downloading"),
                    Some(ProgressDetail {
                        current: Some(c),
                        total: Some(t),
                    }),
                ) => {
                    let percent = *c as f64 / *t as f64 * 100.0;
                    let progress = (percent as i64 / 10) as usize;
                    let remaining = 10 - progress;
                    format!("{:=<progress$}>{:remaining$}   {percent:.0}%", "", "")
                }
                (Some(status), _) => status.to_string(),
                _ => "Unknown".to_string(),
            };
            println!("[{id} {text}]");
        } else {
            println!(
                "{}",
                info.status.as_ref().expect("image info to have a status")
            )
        }
    }
    stdout()
        .queue(MoveUp(
            layers.len().try_into().expect("to convert usize to u16"),
        ))
        .expect("to reset cursor position");
}

struct EngineConfig {
    r#type: String,
    image: String,
    engine: String,
    username: String,
    password: String,
    database_name: String,
    port: String,
    env: Option<Vec<String>>,
    is_ready_cmd: Vec<String>,
}

struct DynamoDbConfig {
    container_name: String,
    image: String,
    port: String,
    aws_access_key_id: String,
    aws_secret_access_key: String,
    aws_default_region: String,
}

fn dynamodb_config() -> DynamoDbConfig {
    DynamoDbConfig {
        container_name: "shuttle_dynamodb".to_string(),
        image: "amazon/dynamodb-local:latest".to_string(),
        port: "8000/tcp".to_string(),
        aws_access_key_id: "DUMMY_ID_EXAMPLE".to_string(),
        aws_secret_access_key: "DUMMY_EXAMPLE_KEY".to_string(),
        aws_default_region: "DUMMY_EXAMPLE_REGION".to_string(),
    }
}

fn db_type_to_config(db_type: Type) -> EngineConfig {
    match db_type {
        Type::Shared(SharedEngine::Postgres) => EngineConfig {
            r#type: "shared_postgres".to_string(),
            image: "docker.io/library/postgres:11".to_string(),
            engine: "postgres".to_string(),
            username: "postgres".to_string(),
            password: "postgres".to_string(),
            database_name: "postgres".to_string(),
            port: "5432/tcp".to_string(),
            env: Some(vec!["POSTGRES_PASSWORD=postgres".to_string()]),
            is_ready_cmd: vec![
                "/bin/sh".to_string(),
                "-c".to_string(),
                "pg_isready | grep 'accepting connections'".to_string(),
            ],
        },
        Type::Shared(SharedEngine::MongoDb) => EngineConfig {
            r#type: "shared_mongodb".to_string(),
            image: "docker.io/library/mongo:5.0.10".to_string(),
            engine: "mongodb".to_string(),
            username: "mongodb".to_string(),
            password: "password".to_string(),
            database_name: "admin".to_string(),
            port: "27017/tcp".to_string(),
            env: Some(vec![
                "MONGO_INITDB_ROOT_USERNAME=mongodb".to_string(),
                "MONGO_INITDB_ROOT_PASSWORD=password".to_string(),
            ]),
            is_ready_cmd: vec![
                "mongosh".to_string(),
                "--quiet".to_string(),
                "--eval".to_string(),
                "db".to_string(),
            ],
        },
        Type::AwsRds(AwsRdsEngine::Postgres) => EngineConfig {
            r#type: "aws_rds_postgres".to_string(),
            image: "docker.io/library/postgres:13.4".to_string(),
            engine: "postgres".to_string(),
            username: "postgres".to_string(),
            password: "postgres".to_string(),
            database_name: "postgres".to_string(),
            port: "5432/tcp".to_string(),
            env: Some(vec!["POSTGRES_PASSWORD=postgres".to_string()]),
            is_ready_cmd: vec![
                "/bin/sh".to_string(),
                "-c".to_string(),
                "pg_isready | grep 'accepting connections'".to_string(),
            ],
        },
        Type::AwsRds(AwsRdsEngine::MariaDB) => EngineConfig {
            r#type: "aws_rds_mariadb".to_string(),
            image: "docker.io/library/mariadb:10.6.7".to_string(),
            engine: "mariadb".to_string(),
            username: "root".to_string(),
            password: "mariadb".to_string(),
            database_name: "mysql".to_string(),
            port: "3306/tcp".to_string(),
            env: Some(vec!["MARIADB_ROOT_PASSWORD=mariadb".to_string()]),
            is_ready_cmd: vec![
                "mysql".to_string(),
                "-pmariadb".to_string(),
                "--silent".to_string(),
                "-e".to_string(),
                "show databases;".to_string(),
            ],
        },
        Type::AwsRds(AwsRdsEngine::MySql) => EngineConfig {
            r#type: "aws_rds_mysql".to_string(),
            image: "docker.io/library/mysql:8.0.28".to_string(),
            engine: "mysql".to_string(),
            username: "root".to_string(),
            password: "mysql".to_string(),
            database_name: "mysql".to_string(),
            port: "3306/tcp".to_string(),
            env: Some(vec!["MYSQL_ROOT_PASSWORD=mysql".to_string()]),
            is_ready_cmd: vec![
                "mysql".to_string(),
                "-pmysql".to_string(),
                "--silent".to_string(),
                "-e".to_string(),
                "show databases;".to_string(),
            ],
        },
    }
}

#[cfg(test)]
mod tests {
    use aws_sdk_dynamodb::error::SdkError;
    use aws_sdk_dynamodb::operation::create_table::CreateTableError;
    use aws_sdk_dynamodb::operation::create_table::CreateTableOutput;
    use aws_sdk_dynamodb::operation::scan::ScanError;
    use aws_sdk_dynamodb::operation::scan::ScanOutput;
    use aws_sdk_dynamodb::types::AttributeDefinition;
    use aws_sdk_dynamodb::types::KeySchemaElement;
    use aws_sdk_dynamodb::types::KeyType;
    use aws_sdk_dynamodb::types::ProvisionedThroughput;
    use aws_sdk_dynamodb::types::ScalarAttributeType;

    use crate::provisioner_server::LocalProvisioner;

    async fn create_table(
        dynamodb_client: &aws_sdk_dynamodb::Client,
        table_name: &str,
        attribute_name: &str,
    ) -> Result<CreateTableOutput, SdkError<CreateTableError>> {
        let attribute_definition = AttributeDefinition::builder()
            .attribute_name(attribute_name)
            .attribute_type(ScalarAttributeType::S)
            .build();

        let key_schema = KeySchemaElement::builder()
            .attribute_name(attribute_name)
            .key_type(KeyType::Hash)
            .build();

        let provisioned_throughput = ProvisionedThroughput::builder()
            .read_capacity_units(10)
            .write_capacity_units(5)
            .build();

        dynamodb_client
            .create_table()
            .table_name(table_name)
            .key_schema(key_schema)
            .attribute_definitions(attribute_definition)
            .provisioned_throughput(provisioned_throughput)
            .send()
            .await
    }

    async fn select_from_table(
        dynamodb_client: &aws_sdk_dynamodb::Client,
        table_name: &str,
    ) -> Result<ScanOutput, SdkError<ScanError>> {
        dynamodb_client.scan().table_name(table_name).send().await
    }

    #[tokio::test]
    async fn test_create_and_delete_dynamodb() {
        let provisioner = LocalProvisioner::new().unwrap();

        let project_name = "test_create_and_delete_dynamodb".to_string();

        let info = provisioner
            .get_dynamodb_connection_info(&project_name)
            .await
            .unwrap();

        std::env::set_var("AWS_ACCESS_KEY_ID", info.aws_access_key_id);
        std::env::set_var("AWS_SECRET_ACCESS_KEY", info.aws_secret_access_key);
        std::env::set_var("AWS_REGION", info.aws_default_region);

        let aws_config = aws_config::from_env()
            .endpoint_url(info.endpoint.unwrap())
            .load()
            .await;

        // create dynamodb client
        let dynamodb_client = aws_sdk_dynamodb::Client::new(&aws_config);

        // create dynamodb table
        let table_name = format!("{}-table", &project_name);
        let attribute_name = format!("{}-attribute", &project_name);
        create_table(&dynamodb_client, &table_name, &attribute_name)
            .await
            .unwrap();

        // select from table (should work)
        let result = select_from_table(&dynamodb_client, &table_name)
            .await
            .unwrap();

        println!("{result:?}");

        // delete dynamodb resource
        provisioner
            .delete_dynamodb_tables_by_prefix_in_container(&project_name)
            .await
            .unwrap();

        // select from table (should fail now that tables have been deleted)
        let result = select_from_table(&dynamodb_client, &table_name).await;

        assert!(
            result.is_err(),
            "expected result to be an error, but found: {result:?}"
        );
    }
}
