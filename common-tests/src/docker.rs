use portpicker::pick_unused_port;
use std::{
    process::Command,
    thread::sleep,
    time::{Duration, SystemTime},
};

pub struct DockerInstance {
    pub container_name: String,
    pub uri: String,
}

struct Config<'a> {
    container_name: String,
    image: &'a str,
    engine: &'a str,
    port: &'a str,
    env: Vec<&'a str>,
    is_ready_cmd: Vec<String>,
}

impl DockerInstance {
    pub fn new(container_name: String) -> Self {
        let Config {
            engine,
            env,
            image,
            is_ready_cmd,
            port,
            container_name,
        } = Config {
            container_name: container_name.clone(),
            // The postgres version should always be in sync with the prod RDS version.
            image: "docker.io/library/postgres:15",
            engine: "postgres",
            port: "5432",
            env: vec!["POSTGRES_PASSWORD=password", "PGUSER=postgres"],
            is_ready_cmd: vec![
                String::from("exec"),
                container_name,
                String::from("pg_isready"),
            ],
        };

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
            uri: format!("{engine}://{engine}:password@localhost:{host_port}"),
        }
    }
}

impl DockerInstance {
    fn wait_ready(mut timeout: Duration, is_ready_cmd: &[String]) {
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
            .args(["stop", &self.container_name])
            .output()
            .expect("failed to stop provisioner test DB container");
        Command::new("docker")
            .args(["rm", &self.container_name])
            .output()
            .expect("failed to remove provisioner test DB container");
    }
}

/// Execute queries in `psql` via `docker exec`
pub fn exec_psql(container_name: &str, query: &str) -> String {
    let args = [
        "exec",
        container_name,
        "psql",
        "--username",
        "postgres",
        "--command",
        query,
    ];
    let output = Command::new("docker").args(args).output().unwrap().stdout;

    String::from_utf8(output).unwrap().trim().to_string()
}
