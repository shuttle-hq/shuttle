use portpicker::pick_unused_port;
use std::{
    process::Command,
    thread::sleep,
    time::{Duration, SystemTime},
};
use uuid::Uuid;

/// A docker instance for a postgres database. It should be used
/// by tests as a singleton (e.g. once_cell::sync::Lazy), and any
/// test logic that connects to it should separate cases by creating
/// an unique database. Also, the instance should implement a destructor.
///
/// Example usage:
///
/// static PG: Lazy<PostgresDockerInstance> = Lazy::new(PostgresDockerInstance::default);

/// #[dtor]
/// fn cleanup() {
///    PG.cleanup();
/// }
///
///  #[tokio::test]
/// async fn test_case() {
///     // Create a unique database name so we have a new database for each test.
///     let db_name = Uuid::new_v4().to_string();
///     let db_uri = PG.get_unique_uri(db_name.as_str());
///     
///     // Test logic below, which can use `db_uri` to connect to the postgres instance.
/// }
///
pub struct DockerInstance {
    container_name: String,
    base_uri: String,
}

impl Default for DockerInstance {
    fn default() -> Self {
        let s = Uuid::new_v4().to_string();
        DockerInstance::new(s.as_str())
    }
}

impl DockerInstance {
    /// Create a new postgres docker instance.
    pub fn new(name: &str) -> Self {
        let container_name = format!("shuttle_test_pg_{}", name);
        let engine = "postgres";
        let env = ["POSTGRES_PASSWORD=password", "PGUSER=postgres"];
        let port = "5432";
        let image = "docker.io/library/postgres:15";
        let is_ready_cmd = vec!["exec", container_name.as_str(), "pg_isready"];
        let host_port = pick_unused_port().unwrap();
        let port_binding = format!("{}:{}", host_port, port);

        let mut args = vec![
            "run",
            "--rm",
            "--name",
            container_name.as_str(),
            "-p",
            &port_binding,
        ];

        args.extend(env.iter().flat_map(|e| ["-e", e]));

        args.push(image);

        Command::new("docker").args(args).spawn().unwrap();

        Self::wait_ready(Duration::from_secs(120), &is_ready_cmd);

        // The container enters the ready state and then reboots, sleep a little and then
        // check if it's ready again afterwards.
        sleep(Duration::from_millis(350));
        Self::wait_ready(Duration::from_secs(120), &is_ready_cmd);

        Self {
            container_name,
            base_uri: format!("{engine}://{engine}:password@localhost:{host_port}"),
        }
    }

    fn wait_ready(mut timeout: Duration, is_ready_cmd: &[&str]) {
        let mut now = SystemTime::now();
        while !timeout.is_zero() {
            let status = Command::new("docker")
                .args(is_ready_cmd)
                .output()
                .unwrap()
                .status;

            if status.success() {
                println!("{is_ready_cmd:?} succeeded...");
                return;
            }

            println!("{is_ready_cmd:?} did not succeed this time...");
            sleep(Duration::from_millis(350));

            timeout = timeout
                .checked_sub(now.elapsed().unwrap())
                .unwrap_or_default();
            now = SystemTime::now();
        }
        panic!("timed out while waiting for provisioner DB to come up");
    }

    // Remove the docker container.
    pub fn cleanup(&self) {
        Command::new("docker")
            .args(["stop", self.container_name.as_str()])
            .output()
            .expect("failed to stop provisioner test DB container");
        Command::new("docker")
            .args(["rm", self.container_name.as_str()])
            .output()
            .expect("failed to remove provisioner test DB container");
    }

    /// This endpoint should be used to get a unique connection string from
    /// the docker instance, so that the instance can be used by multiple
    /// clients in parallel, accessing different databases.
    pub fn get_unique_uri(&self) -> String {
        // Get the PG uri first so the static PG is initialized.
        let db_name = Uuid::new_v4().to_string();
        self.exec_psql(&format!(r#"CREATE DATABASE "{}";"#, db_name));
        format!("{}/{}", &self.base_uri, db_name)
    }

    // Execute queries in `psql` via `docker exec`
    fn exec_psql(&self, query: &str) -> String {
        let args = [
            "exec",
            self.container_name.as_str(),
            "psql",
            "--username",
            "postgres",
            "--command",
            query,
        ];
        let output = Command::new("docker").args(args).output().unwrap().stdout;

        String::from_utf8(output).unwrap().trim().to_string()
    }
}
