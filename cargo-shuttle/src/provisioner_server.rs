use std::{
    collections::HashMap, convert::Infallible, io::stderr, net::SocketAddr, process::exit,
    sync::Arc, time::Duration,
};

use anyhow::{bail, Context, Result};
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
use http_body_util::{combinators::BoxBody, BodyExt, Empty, Full};
use hyper::{
    body::{self, Bytes},
    server::conn::http1,
    service::service_fn,
    Method, Request as HyperRequest, Response,
};
use hyper_util::rt::TokioIo;
use portpicker::pick_unused_port;
use shuttle_common::{
    models::resource::{
        self, ProvisionResourceRequest, ResourceResponse, ResourceState, ResourceType,
    },
    secrets::Secret,
    tables::get_resource_tables,
    ContainerRequest, ContainerResponse, DatabaseInfo, DbInput,
};
use tokio::{net::TcpListener, time::sleep};
use tracing::{debug, error, trace};

/// A provisioner for local runs
/// It uses Docker to create Databases
pub struct LocalProvisioner {
    docker: Docker,
}

impl LocalProvisioner {
    pub fn new() -> Result<Self> {
        // This only constructs the client and does not try to connect.
        // If the socket is not found, a "no such file" error will happen on the first request to Docker.
        Ok(Self {
            docker: Docker::connect_with_defaults()?,
        })
    }

    fn get_container_first_host_port(
        &self,
        container: &ContainerInspectResponse,
        port: &str,
    ) -> String {
        container
            .host_config
            .as_ref()
            .expect("container to have host config")
            .port_bindings
            .as_ref()
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
        container: &ContainerInspectResponse,
        container_type: &str,
        name: &str,
    ) {
        if !container
            .state
            .as_ref()
            .expect("container to have a state")
            .running
            .expect("state to have a running key")
        {
            trace!("{container_type} container '{name}' not running, so starting it");
            self.docker
                .start_container(name, None::<StartContainerOptions<String>>)
                .await
                .expect("failed to start container");
        }
    }

    async fn get_container(
        &self,
        container_name: &str,
        image: &str,
        port: &str,
        env: Option<Vec<String>>,
    ) -> Result<ContainerInspectResponse> {
        match self.docker.inspect_container(container_name, None).await {
            Ok(container) => {
                trace!("found container {container_name}");
                Ok(container)
            }
            Err(bollard::errors::Error::DockerResponseServerError {
                status_code: 404, ..
            }) => {
                self.pull_image(image).await.expect("failed to pull image");
                trace!("will create container {container_name}");
                let options = Some(CreateContainerOptions {
                    name: container_name,
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
                error!("Got unexpected error while inspecting docker container: {error}");
                error!(
                    "Make sure Docker is installed and running. For more help: https://docs.shuttle.dev/docs/local-run#docker-engines"
                );
                Err(anyhow::anyhow!("{}", error))
            }
        }
    }

    async fn get_db_connection_string(
        &self,
        project_name: &str,
        db_type: ResourceType,
        db_name: Option<String>,
    ) -> Result<DatabaseInfo> {
        trace!("getting sql string for project '{project_name}'");

        let database_name = match db_type {
            ResourceType::DatabaseAwsRdsPostgres
            | ResourceType::DatabaseAwsRdsMySql
            | ResourceType::DatabaseAwsRdsMariaDB => {
                db_name.unwrap_or_else(|| project_name.to_string())
            }
            _ => project_name.to_string(),
        };

        let EngineConfig {
            r#type,
            image,
            engine,
            username,
            password,
            port,
            env,
            is_ready_cmd,
        } = db_type_to_config(db_type, &database_name);
        let container_name = format!("shuttle_{project_name}_{type}");

        let container = self
            .get_container(&container_name, &image, &port, env)
            .await?;

        let host_port = self.get_container_first_host_port(&container, &port);

        self.start_container_if_not_running(&container, &r#type, &container_name)
            .await;

        self.wait_for_ready(&container_name, is_ready_cmd.clone())
            .await?;

        // The container enters the ready state, runs an init script and then reboots, so we sleep
        // a little and then check if it's ready again afterwards.
        sleep(Duration::from_millis(450)).await;
        self.wait_for_ready(&container_name, is_ready_cmd).await?;

        let res = DatabaseInfo::new(
            engine,
            username,
            password.expose().clone(),
            database_name,
            host_port,
            "localhost".to_string(),
            None,
        );

        Ok(res)
    }

    pub async fn start_container(&self, req: ContainerRequest) -> Result<ContainerResponse> {
        let ContainerRequest {
            project_name,
            container_name,
            env,
            image,
            port,
        } = req;

        let container_name = format!("shuttle_{project_name}_{container_name}");

        let container = self
            .get_container(&container_name, &image, &port, Some(env))
            .await?;

        let host_port = self.get_container_first_host_port(&container, &port);

        self.start_container_if_not_running(&container, &container_name, &container_name)
            .await;

        Ok(ContainerResponse { host_port })
    }

    async fn wait_for_ready(&self, container_name: &str, is_ready_cmd: Vec<String>) -> Result<()> {
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
        stderr()
            .queue(MoveDown(
                layers.len().try_into().expect("to convert usize to u16"),
            ))
            .expect("to reset cursor position");

        Ok(())
    }
}

fn print_layers(layers: &Vec<CreateImageInfo>) {
    for info in layers {
        stderr()
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
    stderr()
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
    password: Secret<String>,
    port: String,
    env: Option<Vec<String>>,
    is_ready_cmd: Vec<String>,
}

fn db_type_to_config(db_type: ResourceType, database_name: &str) -> EngineConfig {
    match db_type {
        ResourceType::DatabaseSharedPostgres => EngineConfig {
            r#type: "shared_postgres".to_string(),
            image: "docker.io/library/postgres:16".to_string(),
            engine: "postgres".to_string(),
            username: "postgres".to_string(),
            password: "postgres".to_string().into(),
            port: "5432/tcp".to_string(),
            env: Some(vec![
                "POSTGRES_PASSWORD=postgres".to_string(),
                format!("POSTGRES_DB={database_name}"),
            ]),
            is_ready_cmd: vec![
                "/bin/sh".to_string(),
                "-c".to_string(),
                "pg_isready | grep 'accepting connections'".to_string(),
            ],
        },
        ResourceType::DatabaseAwsRdsPostgres => EngineConfig {
            r#type: "aws_rds_postgres".to_string(),
            image: "docker.io/library/postgres:16".to_string(),
            engine: "postgres".to_string(),
            username: "postgres".to_string(),
            password: "postgres".to_string().into(),
            port: "5432/tcp".to_string(),
            env: Some(vec![
                "POSTGRES_PASSWORD=postgres".to_string(),
                format!("POSTGRES_DB={database_name}"),
            ]),
            is_ready_cmd: vec![
                "/bin/sh".to_string(),
                "-c".to_string(),
                "pg_isready | grep 'accepting connections'".to_string(),
            ],
        },
        ResourceType::DatabaseAwsRdsMariaDB => EngineConfig {
            r#type: "aws_rds_mariadb".to_string(),
            image: "docker.io/library/mariadb:10.6.7".to_string(),
            engine: "mariadb".to_string(),
            username: "root".to_string(),
            password: "mariadb".to_string().into(),
            port: "3306/tcp".to_string(),
            env: Some(vec![
                "MARIADB_ROOT_PASSWORD=mariadb".to_string(),
                format!("MARIADB_DATABASE={database_name}"),
            ]),
            is_ready_cmd: vec![
                "mysql".to_string(),
                "-pmariadb".to_string(),
                "--silent".to_string(),
                "-e".to_string(),
                "show databases;".to_string(),
            ],
        },
        ResourceType::DatabaseAwsRdsMySql => EngineConfig {
            r#type: "aws_rds_mysql".to_string(),
            image: "docker.io/library/mysql:8.0.28".to_string(),
            engine: "mysql".to_string(),
            username: "root".to_string(),
            password: "mysql".to_string().into(),
            port: "3306/tcp".to_string(),
            env: Some(vec![
                "MYSQL_ROOT_PASSWORD=mysql".to_string(),
                format!("MYSQL_DATABASE={database_name}"),
            ]),
            is_ready_cmd: vec![
                "mysql".to_string(),
                "-pmysql".to_string(),
                "--silent".to_string(),
                "-e".to_string(),
                "show databases;".to_string(),
            ],
        },
        _ => panic!("Non-database resource type provided: {db_type}"),
    }
}

#[derive(Clone)]
pub struct ProvApiState {
    pub project_name: String,
    pub secrets: HashMap<String, String>,
}

pub struct ProvisionerServer;

impl ProvisionerServer {
    pub async fn run(
        state: Arc<ProvApiState>,
        api_addr: &SocketAddr,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let listener = TcpListener::bind(api_addr).await?;
        loop {
            let (stream, _) = listener.accept().await?;
            let io = TokioIo::new(stream);

            let state = Arc::clone(&state);
            tokio::task::spawn(async move {
                if let Err(err) = http1::Builder::new()
                    .serve_connection(io, service_fn(|req| handler(Arc::clone(&state), req)))
                    .await
                {
                    eprintln!("Provisioner server error: {:?}", err);
                    exit(1);
                }
            });
        }
    }
}

pub async fn handler(
    state: Arc<ProvApiState>,
    req: HyperRequest<body::Incoming>,
) -> std::result::Result<Response<BoxBody<Bytes, Infallible>>, hyper::http::Error> {
    let method = req.method().clone();
    let uri = req.uri().clone();
    debug!("Received {method} {uri}");

    let body = req.into_body().collect().await.unwrap().to_bytes();

    match provision(state, method, uri.to_string().as_str(), body.to_vec()).await {
        Ok(bytes) => Response::builder()
            .status(200)
            .body(BoxBody::new(Full::new(Bytes::from(bytes)))),
        Err(e) => {
            eprintln!("Encountered error when provisioning: {e}");
            Response::builder().status(500).body(Empty::new().boxed())
        }
    }
}

async fn provision(
    state: Arc<ProvApiState>,
    method: Method,
    uri: &str,
    body: Vec<u8>,
) -> Result<Vec<u8>> {
    Ok(match (method, uri) {
        (Method::GET, "/projects/proj_LOCAL/resources/secrets") => {
            let response = ResourceResponse {
                r#type: ResourceType::Secrets,
                state: ResourceState::Ready,
                config: serde_json::Value::Null,
                output: serde_json::to_value(&state.secrets).unwrap(),
            };
            let table = get_resource_tables(&[response.clone()], "local service", false, true);
            println!("{table}");
            serde_json::to_vec(&response).unwrap()
        }
        (Method::POST, "/projects/proj_LOCAL/resources") => {
            let prov = LocalProvisioner::new().unwrap();
            let shuttle_resource: ProvisionResourceRequest =
                serde_json::from_slice(&body).context("deserializing resource request")?;

            let response = match shuttle_resource.r#type {
                ResourceType::DatabaseSharedPostgres
                | ResourceType::DatabaseAwsRdsMariaDB
                | ResourceType::DatabaseAwsRdsMySql
                | ResourceType::DatabaseAwsRdsPostgres => {
                    let config: DbInput = serde_json::from_value(shuttle_resource.config.clone())
                        .context("deserializing resource config")?;
                    let res = prov.get_db_connection_string(
                            &state.project_name,
                            shuttle_resource.r#type,
                            config.db_name,
                        )
                        .await
                        .context("Failed to start database container. Make sure that a Docker engine is running.")?;
                    ResourceResponse {
                        r#type: shuttle_resource.r#type,
                        state: resource::ResourceState::Ready,
                        config: shuttle_resource.config,
                        output: serde_json::to_value(res).unwrap(),
                    }
                }
                ResourceType::Container => {
                    let config = serde_json::from_value(shuttle_resource.config.clone())
                        .context("deserializing resource config")?;
                    let res = prov.start_container(config)
                            .await
                            .context("Failed to start Docker container. Make sure that a Docker engine is running.")?;
                    ResourceResponse {
                        r#type: shuttle_resource.r#type,
                        state: resource::ResourceState::Ready,
                        config: shuttle_resource.config,
                        output: serde_json::to_value(res).unwrap(),
                    }
                }
                ResourceType::Secrets => ResourceResponse {
                    r#type: shuttle_resource.r#type,
                    state: resource::ResourceState::Ready,
                    config: shuttle_resource.config,
                    output: serde_json::to_value(&state.secrets).unwrap(),
                },
                ResourceType::Unknown => bail!("request for unknown resource type recieved"),
            };

            let table = get_resource_tables(&[response.clone()], "local service", false, true);
            println!("{table}");

            serde_json::to_vec(&response).unwrap()
        }
        _ => bail!("Received unsupported resource request"),
    })
}
