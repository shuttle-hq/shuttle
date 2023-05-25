use anyhow::Result;
use async_trait::async_trait;
use bollard::{
    container::{Config, CreateContainerOptions, StartContainerOptions},
    exec::{CreateExecOptions, CreateExecResults},
    image::CreateImageOptions,
    models::{CreateImageInfo, HostConfig, PortBinding, ProgressDetail},
    Docker,
};
use crossterm::{
    cursor::{MoveDown, MoveUp},
    terminal::{Clear, ClearType},
    QueueableCommand,
};
use futures::StreamExt;
use portpicker::pick_unused_port;
use shuttle_common::database::{AwsRdsEngine, SharedEngine};
use shuttle_proto::provisioner::{
    provisioner_server::{Provisioner, ProvisionerServer},
    DatabaseDeletionResponse, DatabaseRequest, DatabaseResponse, DynamoDbRequest, DynamoDbResponse, DynamoDbDeletionResponse,
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

        let container = match self.docker.inspect_container(&container_name, None).await {
            Ok(container) => {
                trace!("found DB container {container_name}");
                container
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
                    port.clone(),
                    Some(vec![PortBinding {
                        host_port: Some(host_port.to_string()),
                        ..Default::default()
                    }]),
                );
                let host_config = HostConfig {
                    port_bindings: Some(port_bindings),
                    ..Default::default()
                };

                let config = Config {
                    image: Some(image),
                    env,
                    host_config: Some(host_config),
                    ..Default::default()
                };

                self.docker
                    .create_container(options, config)
                    .await
                    .expect("to be able to create container");

                self.docker
                    .inspect_container(&container_name, None)
                    .await
                    .expect("container to be created")
            }
            Err(error) => {
                error!("got unexpected error while inspecting docker container: {error}");
                return Err(Status::internal(error.to_string()));
            }
        };

        let port = container
            .host_config
            .expect("container to have host config")
            .port_bindings
            .expect("port bindings on container")
            .get(&port)
            .expect("a port bindings entry")
            .as_ref()
            .expect("a port bindings")
            .first()
            .expect("at least one port binding")
            .host_port
            .as_ref()
            .expect("a host port")
            .clone();

        if !container
            .state
            .expect("container to have a state")
            .running
            .expect("state to have a running key")
        {
            trace!("DB container '{container_name}' not running, so starting it");
            self.docker
                .start_container(&container_name, None::<StartContainerOptions<String>>)
                .await
                .expect("failed to start none running container");
        }

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
        todo!() // possibly using AWS' local dynamodb docker image
    }

    async fn delete_dynamo_db(
        &self,
        request: Request<DynamoDbRequest>,
    ) -> Result<Response<DynamoDbDeletionResponse>, Status> {
        todo!() // possibly using AWS' local dynamodb docker image
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
