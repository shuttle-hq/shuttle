use std::{process::Command, thread::sleep, time::Duration};

use ctor::{ctor, dtor};
use provisioner::MyProvisioner;

const CONTAINER_NAME: &str = "shuttle_provisioner_it";
const PG_URI: &str = "postgres://postgres:password@localhost";

#[ctor]
fn setup() {
    Command::new("docker")
        .args([
            "run",
            "--rm",
            "--name",
            CONTAINER_NAME,
            "-e",
            "POSTGRES_PASSWORD=password",
            "-p",
            "5432:5432",
            "postgres:11",
        ])
        .spawn()
        .unwrap();

    // Wait for it to come up
    loop {
        let status = Command::new("docker")
            .args(["exec", CONTAINER_NAME, "pg_isready"])
            .output()
            .unwrap()
            .status;

        if status.success() {
            break;
        }

        sleep(Duration::from_millis(350));
    }
}

#[dtor]
fn cleanup() {
    Command::new("docker")
        .args(["stop", CONTAINER_NAME])
        .output()
        .expect("failed to stop provisioner test DB container");
    Command::new("docker")
        .args(["rm", CONTAINER_NAME])
        .output()
        .expect("failed to remove provisioner test DB container");
}

fn exec(query: &str) -> String {
    let output = Command::new("docker")
        .args([
            "exec",
            CONTAINER_NAME,
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
    let provisioner = MyProvisioner::new(PG_URI).unwrap();

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
    let provisioner = MyProvisioner::new(PG_URI).unwrap();

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
    let provisioner = MyProvisioner::new(PG_URI).unwrap();

    provisioner
        .request_shared_db("new\"; CREATE ROLE \"injected")
        .await
        .unwrap();
}

#[tokio::test]
async fn shared_db_missing() {
    let provisioner = MyProvisioner::new(PG_URI).unwrap();

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
    let provisioner = MyProvisioner::new(PG_URI).unwrap();

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
