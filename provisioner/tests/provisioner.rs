use portpicker::pick_unused_port;
use std::{
    process::Command,
    thread::sleep,
    time::{Duration, SystemTime},
};

use ctor::dtor;
use lazy_static::lazy_static;
use provisioner::MyProvisioner;

lazy_static! {
    static ref PG: DockerPG = DockerPG::new();
}

#[dtor]
fn cleanup() {
    PG.cleanup();
}

struct DockerPG {
    container_name: String,
    uri: String,
}

impl DockerPG {
    fn new() -> Self {
        let container_name = "shuttle_provisioner_it";
        let port = pick_unused_port().unwrap();

        Command::new("docker")
            .args([
                "run",
                "--rm",
                "--name",
                container_name,
                "-e",
                "POSTGRES_PASSWORD=password",
                "-p",
                &format!("{port}:5432"),
                "postgres:11",
            ])
            .spawn()
            .unwrap();

        Self::wait_ready(container_name, Duration::from_secs(120));

        Self {
            container_name: container_name.to_string(),
            uri: format!("postgres://postgres:password@localhost:{port}"),
        }
    }

    fn wait_ready(container_name: &str, mut timeout: Duration) {
        let mut now = SystemTime::now();
        while !timeout.is_zero() {
            let status = Command::new("docker")
                .args(["exec", container_name, "pg_isready"])
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

    fn cleanup(&self) {
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

fn exec(query: &str) -> String {
    let output = Command::new("docker")
        .args([
            "exec",
            &PG.container_name,
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

#[tokio::test]
async fn shared_db_role_does_not_exist() {
    let provisioner = MyProvisioner::new(&PG.uri).await.unwrap();

    assert_eq!(
        exec("SELECT rolname FROM pg_roles WHERE rolname = 'user-not_exist'"),
        ""
    );

    provisioner.request_shared_db("not_exist").await.unwrap();

    assert_eq!(
        exec("SELECT rolname FROM pg_roles WHERE rolname = 'user-not_exist'"),
        "user-not_exist"
    );
}

#[tokio::test]
async fn shared_db_role_does_exist() {
    let provisioner = MyProvisioner::new(&PG.uri).await.unwrap();

    exec("CREATE ROLE \"user-exist\" WITH LOGIN PASSWORD 'temp'");
    assert_eq!(
        exec("SELECT passwd FROM pg_shadow WHERE usename = 'user-exist'"),
        "md5d44ae85dd21bda2a4f9946217adea2cc"
    );

    provisioner.request_shared_db("exist").await.unwrap();

    // Make sure password got cycled
    assert_ne!(
        exec("SELECT passwd FROM pg_shadow WHERE usename = 'user-exist'"),
        "md5d44ae85dd21bda2a4f9946217adea2cc"
    );
}

#[tokio::test]
#[should_panic(
    expected = "CreateRole(\"error returned from database: cannot insert multiple commands into a prepared statement\""
)]
async fn injection_safe() {
    let provisioner = MyProvisioner::new(&PG.uri).await.unwrap();

    provisioner
        .request_shared_db("new\"; CREATE ROLE \"injected")
        .await
        .unwrap();
}

#[tokio::test]
async fn shared_db_missing() {
    let provisioner = MyProvisioner::new(&PG.uri).await.unwrap();

    assert_eq!(
        exec("SELECT datname FROM pg_database WHERE datname = 'db-missing'"),
        ""
    );

    provisioner.request_shared_db("missing").await.unwrap();

    assert_eq!(
        exec("SELECT datname FROM pg_database WHERE datname = 'db-missing'"),
        "db-missing"
    );
}

#[tokio::test]
async fn shared_db_filled() {
    let provisioner = MyProvisioner::new(&PG.uri).await.unwrap();

    exec("CREATE ROLE \"user-filled\" WITH LOGIN PASSWORD 'temp'");
    exec("CREATE DATABASE \"db-filled\" OWNER 'user-filled'");
    assert_eq!(
        exec("SELECT datname FROM pg_database WHERE datname = 'db-filled'"),
        "db-filled"
    );

    provisioner.request_shared_db("filled").await.unwrap();

    assert_eq!(
        exec("SELECT datname FROM pg_database WHERE datname = 'db-filled'"),
        "db-filled"
    );
}
