use anyhow::Result;
use async_trait::async_trait;
use bollard::{
    container::{Config, CreateContainerOptions, StartContainerOptions},
    exec::{CreateExecOptions, CreateExecResults},
    image::CreateImageOptions,
    models::{CreateImageInfo, HostConfig, PortBinding, ProgressDetail},
    Docker,
};
use colored::Colorize;
use crossterm::{
    cursor::MoveUp,
    terminal::{Clear, ClearType},
    QueueableCommand,
};
use futures::StreamExt;
use portpicker::pick_unused_port;
use shuttle_common::{project::ProjectName, DatabaseReadyInfo};
use shuttle_service::{database::Type, error::CustomError, Factory};
use std::{collections::HashMap, io::stdout, time::Duration};
use tokio::time::sleep;

pub struct LocalFactory {
    docker: Docker,
    project: ProjectName,
}

impl LocalFactory {
    pub fn new(project: ProjectName) -> Result<Self> {
        Ok(Self {
            docker: Docker::connect_with_local_defaults()?,
            project,
        })
    }
}

const PG_PASSWORD: &str = "password";
const PG_IMAGE: &str = "postgres:11";

#[async_trait]
impl Factory for LocalFactory {
    async fn get_sql_connection_string(
        &mut self,
        db_type: Type,
    ) -> Result<String, shuttle_service::Error> {
        trace!("getting sql string for project '{}'", self.project);
        let container_name = format!("shuttle_{}_postgres", self.project);

        let container = match self.docker.inspect_container(&container_name, None).await {
            Ok(container) => {
                trace!("found DB container {container_name}");
                container
            }
            Err(bollard::errors::Error::DockerResponseServerError { status_code, .. })
                if status_code == 404 =>
            {
                self.pull_image(PG_IMAGE)
                    .await
                    .expect("failed to pull image");
                trace!("will create DB container {container_name}");
                let options = Some(CreateContainerOptions {
                    name: container_name.clone(),
                });
                let mut port_bindings = HashMap::new();
                let host_port = pick_unused_port().expect("system to have a free port");
                port_bindings.insert(
                    "5432/tcp".to_string(),
                    Some(vec![PortBinding {
                        host_port: Some(host_port.to_string()),
                        ..Default::default()
                    }]),
                );
                let host_config = HostConfig {
                    port_bindings: Some(port_bindings),
                    ..Default::default()
                };

                let password_env = format!("POSTGRES_PASSWORD={PG_PASSWORD}");
                let config = Config {
                    image: Some(PG_IMAGE),
                    env: Some(vec![&password_env]),
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
                return Err(shuttle_service::Error::Custom(CustomError::new(error)));
            }
        };

        let port = container
            .host_config
            .expect("container to have host config")
            .port_bindings
            .expect("port bindings on container")
            .get("5432/tcp")
            .expect("a '5432/tcp' port bindings entry")
            .as_ref()
            .expect("a '5432/tcp' port bindings")
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

        self.wait_for_ready(&container_name).await?;

        let db_info = DatabaseReadyInfo::new(
            "postgres".to_string(),
            "postgres".to_string(),
            PG_PASSWORD.to_string(),
            "postgres".to_string(),
            port,
            "localhost".to_string(),
            "localhost".to_string(),
        );

        let conn_str = db_info.connection_string_private();

        println!(
            "{:>12} can be reached at {}\n",
            "DB ready".bold().cyan(),
            conn_str
        );

        Ok(conn_str)
    }
}

impl LocalFactory {
    async fn wait_for_ready(&self, container_name: &str) -> Result<(), shuttle_service::Error> {
        loop {
            trace!("waiting for '{container_name}' to be ready for connections");

            let config = CreateExecOptions {
                cmd: Some(vec!["pg_isready"]),
                attach_stdout: Some(true),
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
                    if let bollard::container::LogOutput::StdOut { message } =
                        line.expect("output to have a log line")
                    {
                        if message.ends_with(b"accepting connections\n") {
                            return Ok(());
                        }
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

        Ok(())
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
            println!("[{id} {}]", text);
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
