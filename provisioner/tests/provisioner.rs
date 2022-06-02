use std::{process::Command, time::Duration};

use provisioner::MyProvisioner;
use tokio::time::sleep;

const CONTAINER_NAME: &str = "shuttle_provisioner_it";

struct DockerDB {
    uri: Option<String>,
}

impl DockerDB {
    fn new() -> Self {
        Self { uri: None }
    }

    async fn get_uri(&mut self) -> String {
        if let Some(uri) = self.uri.as_ref() {
            return uri.clone();
        }

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

            sleep(Duration::from_millis(350)).await;
        }

        let uri = "postgres://postgres:password@localhost".to_string();
        self.uri = Some(uri.clone());

        uri
    }

    fn exec(&self, query: &str) -> String {
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
}

impl Drop for DockerDB {
    fn drop(&mut self) {
        if self.uri.is_some() {
            Command::new("docker")
                .args(["stop", CONTAINER_NAME])
                .output()
                .expect("failed to stop provisioner test DB container");
            Command::new("docker")
                .args(["rm", CONTAINER_NAME])
                .output()
                .expect("failed to remove provisioner test DB container");
        }
    }
}

#[tokio::test]
async fn shared_db_does_not_exist() {
    let mut db = DockerDB::new();
    let provisioner = MyProvisioner::new(db.get_uri().await).unwrap();

    assert_eq!(
        db.exec("SELECT rolname FROM pg_roles WHERE rolname = 'not_exist'"),
        ""
    );

    provisioner.request_shared_db("not_exist".to_string()).await;

    assert_eq!(
        db.exec("SELECT rolname FROM pg_roles WHERE rolname = 'not_exist'"),
        "not_exist"
    );
}
