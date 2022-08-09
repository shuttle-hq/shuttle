use portpicker::pick_unused_port;
use shuttle_proto::provisioner::shared;
use std::{
    process::Command,
    thread::sleep,
    time::{Duration, SystemTime},
};

use ctor::dtor;
use lazy_static::lazy_static;
use shuttle_provisioner::MyProvisioner;

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

lazy_static! {
    static ref MONGODB: DockerMongoDb = DockerMongoDb::new();
}

// TODO: fix this
#[dtor]
fn cleanup_mongo() {
    MONGODB.cleanup_mongo();
}

struct DockerMongoDb {
    container_name: String,
    uri: String,
}

impl DockerMongoDb {
    fn new() -> Self {
        //TODO: what should this be?
        let container_name = "shuttle_provisioner_mongo";
        let port = pick_unused_port().unwrap();

        Command::new("docker")
            .args([
                "run",
                "--rm",
                "--name",
                container_name,
                "-e",
                "MONGO_INITDB_ROOT_USERNAME=mongodb",
                "-e",
                "MONGO_INITDB_ROOT_PASSWORD=password",
                "-p",
                &format!("{port}:27017"),
                "mongo:5.0.10",
            ])
            .spawn()
            .unwrap();

        Self::wait_ready(container_name, Duration::from_secs(120));

        Self {
            container_name: container_name.to_string(),
            uri: format!("mongodb://mongodb:password@localhost:{port}"),
        }
    }

    fn wait_ready(container_name: &str, mut timeout: Duration) {
        let mut now = SystemTime::now();
        while !timeout.is_zero() {
            let status = Command::new("docker")
                .args(["exec", container_name, "mongosh", "--quiet", "--eval", "db"])
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

    fn cleanup_mongo(&self) {
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

// TODO: mongodb exec
fn mongodb_exec(command: &str, database_name: Option<&str>) -> String {
    let output = Command::new("docker")
        .args([
            "exec",
            &MONGODB.container_name,
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

#[tokio::test]
async fn shared_db_role_does_not_exist() {
    let provisioner = MyProvisioner::new(
        &PG.uri,
        &MONGODB.uri,
        "fqdn".to_string(),
        "internal".to_string(),
    )
    .await
    .unwrap();

    assert_eq!(
        exec("SELECT rolname FROM pg_roles WHERE rolname = 'user-not_exist'"),
        ""
    );

    provisioner
        .request_shared_db("not_exist", shared::Engine::Postgres(String::new()))
        .await
        .unwrap();

    assert_eq!(
        exec("SELECT rolname FROM pg_roles WHERE rolname = 'user-not_exist'"),
        "user-not_exist"
    );
}

#[tokio::test]
async fn shared_db_role_does_exist() {
    let provisioner = MyProvisioner::new(
        &PG.uri,
        &MONGODB.uri,
        "fqdn".to_string(),
        "internal".to_string(),
    )
    .await
    .unwrap();

    exec("CREATE ROLE \"user-exist\" WITH LOGIN PASSWORD 'temp'");
    assert_eq!(
        exec("SELECT passwd FROM pg_shadow WHERE usename = 'user-exist'"),
        "md5d44ae85dd21bda2a4f9946217adea2cc"
    );

    provisioner
        .request_shared_db("exist", shared::Engine::Postgres(String::new()))
        .await
        .unwrap();

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
    let provisioner = MyProvisioner::new(
        &PG.uri,
        &MONGODB.uri,
        "fqdn".to_string(),
        "internal".to_string(),
    )
    .await
    .unwrap();

    provisioner
        .request_shared_db(
            "new\"; CREATE ROLE \"injected",
            shared::Engine::Postgres(String::new()),
        )
        .await
        .unwrap();
}

#[tokio::test]
async fn shared_db_missing() {
    let provisioner = MyProvisioner::new(
        &PG.uri,
        &MONGODB.uri,
        "fqdn".to_string(),
        "internal".to_string(),
    )
    .await
    .unwrap();

    assert_eq!(
        exec("SELECT datname FROM pg_database WHERE datname = 'db-missing'"),
        ""
    );

    provisioner
        .request_shared_db("missing", shared::Engine::Postgres(String::new()))
        .await
        .unwrap();

    assert_eq!(
        exec("SELECT datname FROM pg_database WHERE datname = 'db-missing'"),
        "db-missing"
    );
}

#[tokio::test]
async fn shared_db_filled() {
    let provisioner = MyProvisioner::new(
        &PG.uri,
        &MONGODB.uri,
        "fqdn".to_string(),
        "internal".to_string(),
    )
    .await
    .unwrap();

    exec("CREATE ROLE \"user-filled\" WITH LOGIN PASSWORD 'temp'");
    exec("CREATE DATABASE \"db-filled\" OWNER 'user-filled'");
    assert_eq!(
        exec("SELECT datname FROM pg_database WHERE datname = 'db-filled'"),
        "db-filled"
    );

    provisioner
        .request_shared_db("filled", shared::Engine::Postgres(String::new()))
        .await
        .unwrap();

    assert_eq!(
        exec("SELECT datname FROM pg_database WHERE datname = 'db-filled'"),
        "db-filled"
    );
}

// TODO: more tests
#[tokio::test]
async fn request_shared_mongodb_with_private_role() {
    let provisioner = MyProvisioner::new(
        &PG.uri,
        &MONGODB.uri,
        "fqdn".to_string(),
        "internal".to_string(),
    )
    .await
    .unwrap();

    assert!(!mongodb_exec("'show dbs'", None).contains("mongodb-provisioner-test"));

    provisioner
        .request_shared_db("provisioner-test", shared::Engine::Mongodb(String::new()))
        .await
        .unwrap();

    // `show dbs` command doesn't show DBs without collections, so instead we run the
    // `getUser` command in the new database
    let show_users = mongodb_exec(
        "db.getUser(\"user-provisioner-test\")",
        Some("mongodb-provisioner-test"),
    );

    assert!(show_users.contains("user-provisioner-test"));
    assert!(show_users.contains("role: 'readWrite', db: 'mongodb-provisioner-test'"));
}
