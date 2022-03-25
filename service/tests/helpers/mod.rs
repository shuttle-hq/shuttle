use std::{process::Command, thread::sleep, time::Duration};

use portpicker::pick_unused_port;

pub struct PostgresInstance {
    port: u16,
    container: String,
    password: String,
}

impl PostgresInstance {
    pub fn new() -> Self {
        let port = pick_unused_port().expect("could not find a free port for postgres");
        let container = "postgres-shuttle-service-integration-test".to_string();
        let password = "password".to_string();

        Command::new("docker")
            .args([
                "run",
                "--name",
                &container,
                "-e",
                &format!("POSTGRES_PASSWORD={}", password),
                "-p",
                &format!("{}:5432", port),
                "postgres:11", // Our Dockerfile image is based on buster which has postgres version 11
            ])
            .spawn()
            .expect("failed to start a postgres instance");

        Self::wait_for_up(&container);

        Self {
            port,
            container,
            password,
        }
    }

    pub fn get_uri(&self) -> String {
        format!(
            "postgres://postgres:{}@localhost:{}",
            self.password, self.port
        )
    }

    fn wait_for_up(container: &str) {
        // Docker needs a quick warmup time, else we will catch a ready state prematurely
        sleep(Duration::from_millis(100));

        let mut timeout = 20 * 10;

        while timeout > 0 {
            timeout -= 1;

            let status = Command::new("docker")
                .args(["exec", container, "pg_isready"])
                .output()
                .expect("failed to get postgres ready status")
                .status;

            if status.success() {
                break;
            }

            sleep(Duration::from_millis(100));
        }
    }
}

impl Drop for PostgresInstance {
    fn drop(&mut self) {
        Command::new("docker")
            .args(["stop", &self.container])
            .spawn()
            .expect("failed to spawn stop for postgres container")
            .wait()
            .expect("postgres container stop failed");

        Command::new("docker")
            .args(["rm", &self.container])
            .spawn()
            .expect("failed to spawn stop for remove container")
            .wait()
            .expect("postgres container remove failed");
    }
}
