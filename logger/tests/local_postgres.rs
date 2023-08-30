use std::{
    collections::HashMap,
    future::Future,
    net::{Ipv4Addr, SocketAddr},
    panic,
    process::Command,
    sync::Arc,
    time::Duration,
};

use bollard::{
    container::{Config, CreateContainerOptions, StartContainerOptions},
    errors::Error,
    exec::{CreateExecOptions, CreateExecResults},
    image::CreateImageOptions,
    service::{CreateImageInfo, HealthConfig, HostConfig, PortBinding},
    Docker,
};
use portpicker::pick_unused_port;
use shuttle_common::claims::Scope;
use shuttle_common_tests::JwtScopesLayer;
use shuttle_logger::{Postgres, Service};
use shuttle_proto::logger::logger_server::LoggerServer;
use sqlx::{
    postgres::{PgConnectOptions, PgSslMode},
    PgPool,
};
use tokio::{sync::Mutex, time::sleep};
use tokio_stream::StreamExt;
use tonic::transport::Server;

const CONTAINER_NAME: &str = "logger-postgres";

async fn logger_server(options: PgConnectOptions, port: u16) {
    let addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), port);
    let postgres = Postgres::with_options(options).await;
    Server::builder()
        .layer(JwtScopesLayer::new(vec![Scope::Logs]))
        .add_service(LoggerServer::new(Service::new(
            postgres.get_sender(),
            postgres.clone(),
        )))
        .serve(addr)
        .await
        .unwrap()
}

pub struct LocalPostgresWrapper {
    docker: Arc<Mutex<Docker>>,
    host_port: u16,
}

impl Default for LocalPostgresWrapper {
    fn default() -> Self {
        Self {
            docker: Arc::new(Mutex::new(Docker::connect_with_local_defaults().unwrap())),
            host_port: pick_unused_port().unwrap(),
        }
    }
}

impl LocalPostgresWrapper {
    pub async fn pg_connect_options(&self) -> Result<PgConnectOptions, Error> {
        let image = "docker.io/library/postgres:14".to_string();
        let port = "5432/tcp".to_string();
        let env = Some(vec![
            "POSTGRES_PASSWORD=postgres".to_string(),
            "PGUSER=postgres".to_string(),
        ]);
        let is_ready_cmd = vec![
            "/bin/sh".to_string(),
            "-c".to_string(),
            "pg_isready | grep 'accepting connections'".to_string(),
        ];

        self.pull_image(&image).await.expect("failed to pull image");
        let container_name = format!("{CONTAINER_NAME}{}", self.host_port);

        let docker = self.docker.lock().await;
        let container = match docker.inspect_container(&container_name, None).await {
            Ok(container) => container,
            Err(bollard::errors::Error::DockerResponseServerError { status_code, .. })
                if status_code == 404 =>
            {
                let options = Some(CreateContainerOptions {
                    name: container_name.to_string(),
                    platform: None,
                });

                let mut port_bindings = HashMap::new();
                port_bindings.insert(
                    port.clone(),
                    Some(vec![PortBinding {
                        host_port: Some(self.host_port.to_string()),
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
                    healthcheck: Some(HealthConfig {
                        test: Some(is_ready_cmd.clone()),
                        interval: Some(1000000000),
                        timeout: Some(1000000000),
                        retries: Some(10),
                        start_period: Some(1000000),
                    }),
                    ..Default::default()
                };

                docker
                    .create_container(options, config)
                    .await
                    .expect("to be able to create container");

                docker
                    .inspect_container(&container_name, None)
                    .await
                    .expect("container to be created")
            }
            Err(error) => return Err(error),
        };

        if !container
            .state
            .as_ref()
            .expect("container to have a state")
            .running
            .expect("state to have a running key")
        {
            docker
                .start_container(&container_name, None::<StartContainerOptions<String>>)
                .await
                .expect("failed to start none running container");
        }

        'a: for _ in 0..10 {
            let config = CreateExecOptions {
                cmd: Some(is_ready_cmd.clone()),
                attach_stdout: Some(true),
                attach_stderr: Some(true),
                ..Default::default()
            };

            let CreateExecResults { id } = docker
                .create_exec(&container_name, config)
                .await
                .expect("failed to create exec to check if container is ready");

            let ready_result = docker
                .start_exec(&id, None)
                .await
                .expect("failed to execute ready command");

            if let bollard::exec::StartExecResults::Attached { mut output, .. } = ready_result {
                while let Some(line) = output.next().await {
                    if let bollard::container::LogOutput::StdOut { .. } =
                        line.expect("output to have a log line")
                    {
                        break 'a;
                    }
                }
            }
            sleep(Duration::from_millis(500)).await;
        }

        Ok(PgConnectOptions::default()
            .database("postgres")
            .username("postgres")
            .password("postgres")
            .host("localhost")
            .port(self.host_port)
            .ssl_mode(PgSslMode::Disable))
    }

    async fn pull_image(&self, image: &str) -> Result<(), String> {
        let mut layers = Vec::new();

        let create_image_options = Some(CreateImageOptions {
            from_image: image,
            ..Default::default()
        });

        let mut output = self
            .docker
            .lock()
            .await
            .create_image(create_image_options, None, None);

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
        }

        Ok(())
    }

    pub async fn create_db(&self, db_name: &str) {
        let pool = PgPool::connect_with(self.pg_connect_options().await.unwrap())
            .await
            .unwrap();
        sqlx::query(&format!(
            "CREATE DATABASE \"{}\"",
            db_name.replace('"', "\"\"")
        ))
        .execute(&pool)
        .await
        .unwrap();
    }

    pub async fn teardown_db(&self, db_name: &str) {
        let pool = PgPool::connect_with(self.pg_connect_options().await.unwrap())
            .await
            .unwrap();
        let _ = sqlx::query(&format!(
            "DROP DATABASE IF EXISTS \"{}\" WITH ( FORCE )",
            db_name.replace('"', "\"\""),
        ))
        .execute(&pool)
        .await
        .unwrap();
    }

    pub async fn run_against_underlying_container<Fut>(
        &self,
        test: Fut,
        logger_port: u16,
        db_name: &str,
    ) where
        Fut: Future<Output = ()> + Send + 'static,
    {
        self.create_db(db_name).await;

        // Start server, handing over the postgres connection.
        let options_connect = self.pg_connect_options().await.unwrap().database(db_name);

        let handle = tokio::spawn(logger_server(options_connect, logger_port));
        let result = match tokio::spawn(test).await {
            Ok(_) => Ok(()),
            Err(e) => {
                // Test failed
                Err(e.try_into_panic().ok())
            }
        };

        handle.abort();
        self.teardown_db(db_name).await;
        if let Err(option) = result {
            match option {
                Some(panic) => panic::resume_unwind(panic),
                None => panic!("test future cancelled"),
            };
        }
    }
}

impl Drop for LocalPostgresWrapper {
    fn drop(&mut self) {
        let host_port = self.host_port;
        let _ = Command::new("docker")
            .arg("rm")
            .arg("-f")
            .arg(&format!("{CONTAINER_NAME}{host_port}"))
            .spawn()
            .unwrap()
            .wait()
            .unwrap();
    }
}
