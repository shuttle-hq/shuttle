use std::{
    process::Command,
    thread::sleep,
    time::{Duration, SystemTime},
};

use portpicker::pick_unused_port;

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

pub enum DbType {
    Postgres,
    MongoDb,
}

impl DockerInstance {
    pub fn new(db_type: DbType) -> Self {
        let Config {
            engine,
            env,
            image,
            is_ready_cmd,
            port,
            container_name,
        } = Config::from(db_type);

        let host_port = pick_unused_port().unwrap();
        let port_binding = format!("{}:{}", host_port, port);

        let mut args = vec![
            "run",
            "--rm",
            "--name",
            &container_name,
            "-p",
            &port_binding,
        ];

        args.extend(env.iter().flat_map(|e| ["-e", e]));

        args.push(&image);

        Command::new("docker").args(args).spawn().unwrap();

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
                return;
            }

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

impl From<DbType> for Config<'_> {
    fn from(db_type: DbType) -> Self {
        match db_type {
            DbType::Postgres => Config {
                container_name: "shuttle_provisioner_pg",
                image: "postgres:11",
                engine: "postgres",
                port: "5432",
                env: vec!["POSTGRES_PASSWORD=password"],
                is_ready_cmd: vec!["exec", "shuttle_provisioner_pg", "pg_isready"],
            },
            DbType::MongoDb => Config {
                container_name: "shuttle_provisioner_mongodb",
                image: "mongo:5.0.10",
                engine: "mongodb",
                port: "27017",
                env: vec![
                    "MONGO_INITDB_ROOT_USERNAME=mongodb",
                    "MONGO_INITDB_ROOT_PASSWORD=password",
                ],
                is_ready_cmd: vec![
                    "exec",
                    "shuttle_provisioner_mongodb",
                    "mongosh",
                    "--quiet",
                    "--eval",
                    "db",
                ],
            },
        }
    }
}

pub fn exec_psql(query: &str) -> String {
    let output = Command::new("docker")
        .args([
            "exec",
            "shuttle_provisioner_pg",
            "psql",
            "--username",
            "postgres",
            "--tuples-only",
            "--no-align",
            "--field-separator",
            ",",
            "--command",
            query,
        ])
        .output()
        .unwrap()
        .stdout;

    String::from_utf8(output).unwrap().trim().to_string()
}

pub fn exec_mongosh(command: &str, database_name: Option<&str>) -> String {
    let output = Command::new("docker")
        .args([
            "exec",
            "shuttle_provisioner_mongodb",
            "mongosh",
            "--quiet",
            "--username",
            "mongodb",
            "--password",
            "password",
            "--authenticationDatabase",
            "admin",
            database_name.unwrap_or("admin"),
            "--eval",
            command,
        ])
        .output()
        .unwrap()
        .stdout;

    String::from_utf8(output).unwrap().trim().to_string()
}
