use std::future::Future;
use std::process::Command;
use std::thread::sleep;
use std::time::Duration;

use portpicker::pick_unused_port;
use sqlx::Connection;

pub struct PostgresInstance {
    port: u16,
    container: String,
    password: String,
}

impl PostgresInstance {
    /// Creates a new [`PostgresInstance`] using the official postgres:11 docker image
    ///
    /// Does not wait for the container to be ready. Use [`PostgresInstance::wait_for_ready`] and
    /// [`PostgresInstance::wait_for_connectable`] for that.
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
                "postgres:11", // Our Containerfile image is based on buster which has postgres version 11
            ])
            .spawn()
            .expect("failed to start a postgres instance");

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

    pub fn wait_for_connectable(&self) -> impl Future<Output = ()> + '_ {
        self.async_wait_for(|instance| {
            let uri = instance.get_uri().as_str().to_string();
            async move { sqlx::PgConnection::connect(uri.as_str()).await.is_ok() }
        })
    }

    pub async fn async_wait_for<F, Fut>(&self, f: F)
    where
        F: Fn(&Self) -> Fut,
        Fut: Future<Output = bool>,
    {
        let mut timeout = 20 * 10;

        while timeout > 0 {
            timeout -= 1;

            if f(self).await {
                return;
            }

            sleep(Duration::from_millis(100));
        }

        panic!("timed out waiting for PostgresInstance");
    }

    pub fn wait_for_ready(&self) {
        self.wait_for(|instance| {
            let status = Command::new("docker")
                .args(["exec", &instance.container, "pg_isready"])
                .output()
                .expect("failed to get postgres ready status")
                .status;

            status.success()
        })
    }

    pub fn wait_for<F>(&self, f: F)
    where
        F: Fn(&Self) -> bool,
    {
        let mut timeout = 20 * 10;

        while timeout > 0 {
            timeout -= 1;

            if f(self) {
                return;
            }

            sleep(Duration::from_millis(100));
        }

        panic!("timed out waiting for PostgresInstance");
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
