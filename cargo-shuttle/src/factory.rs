use async_trait::async_trait;
use bollard::{
    container::{Config, CreateContainerOptions, StartContainerOptions},
    exec::{CreateExecOptions, CreateExecResults},
    models::{HostConfig, PortBinding},
    Docker,
};
use colored::Colorize;
use futures::StreamExt;
use shuttle_common::{project::ProjectName, DatabaseReadyInfo};
use shuttle_service::Factory;
use std::{collections::HashMap, time::Duration};
use tokio::time::sleep;

pub struct LocalFactory {
    docker: Docker,
    project: ProjectName,
}

impl LocalFactory {
    pub fn new(project: ProjectName) -> Self {
        Self {
            docker: Docker::connect_with_local_defaults().unwrap(),
            project,
        }
    }
}

#[async_trait]
impl Factory for LocalFactory {
    async fn get_sql_connection_string(&mut self) -> Result<String, shuttle_service::Error> {
        let container_name = format!("shuttle_{}_postgres", self.project);

        let container = match self.docker.inspect_container(&container_name, None).await {
            Ok(container) => {
                trace!("found DB container {container_name}");
                container
            }
            Err(bollard::errors::Error::DockerResponseServerError { status_code, .. })
                if status_code == 404 =>
            {
                trace!("will create DB container {container_name}");
                let options = Some(CreateContainerOptions {
                    name: container_name.clone(),
                });
                let mut port_bindings = HashMap::new();
                port_bindings.insert(
                    "5432".to_string(),
                    Some(vec![PortBinding {
                        host_port: Some("5432".to_string()),
                        ..Default::default()
                    }]),
                );
                let host_config = HostConfig {
                    port_bindings: Some(port_bindings),
                    ..Default::default()
                };

                let config = Config {
                    image: Some("postgres:11"),
                    env: Some(vec!["POSTGRES_PASSWORD=password"]),
                    host_config: Some(host_config),
                    ..Default::default()
                };

                self.docker.create_container(options, config).await.unwrap();

                self.docker
                    .inspect_container(&container_name, None)
                    .await
                    .expect("container to be created")
            }
            error => todo!("unexpected error: {error:?}"),
        };

        if !container.state.unwrap().running.unwrap() {
            trace!("DB container '{container_name}' not running, so starting it");
            self.docker
                .start_container(&container_name, None::<StartContainerOptions<String>>)
                .await
                .expect("failed to start not running container");
        }

        self.wait_for_ready(&container_name).await?;

        let db_info = DatabaseReadyInfo {
            database_name: "postgres".to_string(),
            role_name: "postgres".to_string(),
            role_password: "password".to_string(),
        };

        let conn_str = db_info.connection_string("localhost");

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

            match ready_result {
                bollard::exec::StartExecResults::Attached { mut output, .. } => {
                    while let Some(line) = output.next().await {
                        if let bollard::container::LogOutput::StdOut { message } = line.unwrap() {
                            if message.ends_with(b"accepting connections\n") {
                                return Ok(());
                            }
                        }
                    }
                }
                bollard::exec::StartExecResults::Detached => todo!(),
            }

            sleep(Duration::from_millis(500)).await;
        }
    }
}
