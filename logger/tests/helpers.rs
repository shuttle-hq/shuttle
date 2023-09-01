use portpicker::pick_unused_port;
use std::{
    process::Command,
    thread::sleep,
    time::{Duration, SystemTime},
};

const PG_CONTAINER_NAME: &str = "shuttle_logger_test_pg";

pub struct DockerInstance {
    pub container_name: &'static str,
    pub uri: String,
}

struct Config<'a> {
    container_name: &'a str,
    image: &'a str,
    engine: &'a str,
    port: &'a str,
    env: Vec<&'a str>,
    is_ready_cmd: Vec<&'a str>,
}

impl DockerInstance {
    pub fn new() -> Self {
        let Config {
            engine,
            env,
            image,
            is_ready_cmd,
            port,
            container_name,
        } = Config {
            container_name: PG_CONTAINER_NAME,
            // The postgres version should always be in sync with the prod RDS version.
            image: "docker.io/library/postgres:15",
            engine: "postgres",
            port: "5432",
            env: vec!["POSTGRES_PASSWORD=password", "PGUSER=postgres"],
            is_ready_cmd: vec!["exec", PG_CONTAINER_NAME, "pg_isready"],
        };

        let host_port = pick_unused_port().unwrap();
        let port_binding = format!("{}:{}", host_port, port);

        let mut args = vec!["run", "--rm", "--name", container_name, "-p", &port_binding];

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
            uri: format!("{engine}://{engine}:password@localhost:{host_port}"),
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

    pub fn cleanup(&self) {
        Command::new("docker")
            .args(["stop", self.container_name])
            .output()
            .expect("failed to stop provisioner test DB container");
        Command::new("docker")
            .args(["rm", self.container_name])
            .output()
            .expect("failed to remove provisioner test DB container");
    }
}

/// Execute queries in `psql` via `docker exec`
pub fn exec_psql(query: &str) -> String {
    let args = [
        "exec",
        PG_CONTAINER_NAME,
        "psql",
        "--username",
        "postgres",
        "--command",
        query,
    ];
    let output = Command::new("docker").args(args).output().unwrap().stdout;

    String::from_utf8(output).unwrap().trim().to_string()
}
