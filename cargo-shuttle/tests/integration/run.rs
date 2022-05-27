use cargo_shuttle::{Args, Command, ProjectArgs, Shuttle};
use core::panic;
use reqwest::StatusCode;
use std::{fs::canonicalize, time::Duration};
use tokio::time::sleep;

/// creates a `cargo-shuttle` run instance with some reasonable defaults set.
async fn cargo_shuttle_run(working_directory: &str) {
    let working_directory = canonicalize(working_directory).unwrap();

    let runner = Shuttle::new().run(Args {
        api_url: Some("network support is intentionally broken in tests".to_string()),
        project_args: ProjectArgs {
            working_directory,
            name: None,
        },
        cmd: Command::Run,
    });

    tokio::spawn(async {
        sleep(Duration::from_secs(120)).await;

        panic!("run test took too long. Did it fail to shutdown?");
    });

    tokio::spawn(runner);

    // Wait for service to be responsive
    while let Err(_) = reqwest::Client::new()
        .get("http://localhost:8000")
        .send()
        .await
    {
        sleep(Duration::from_millis(350)).await;
    }
}

#[tokio::test]
async fn rocket_hello_world() {
    cargo_shuttle_run("../examples/rocket/hello-world").await;

    let request_text = reqwest::Client::new()
        .get("http://localhost:8000/hello")
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();

    assert_eq!(request_text, "Hello, world!");
}

#[tokio::test]
async fn rocket_authentication() {
    cargo_shuttle_run("../examples/rocket/authentication").await;
    let client = reqwest::Client::new();

    let public_text = client
        .get("http://localhost:8000/public")
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();

    assert_eq!(
        public_text,
        "{\"message\":\"This endpoint is open to anyone\"}"
    );

    let private_status = client
        .get("http://localhost:8000/private")
        .send()
        .await
        .unwrap()
        .status();

    assert_eq!(private_status, StatusCode::FORBIDDEN);

    let body = client
        .post("http://localhost:8000/login")
        .body("{\"username\": \"username\", \"password\": \"password\"}")
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    let token = format!("Bearer {}", json["token"].as_str().unwrap());

    let private_text = client
        .get("http://localhost:8000/private")
        .header("Authorization", token)
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();

    assert_eq!(
        private_text,
        "{\"message\":\"The `Claims` request guard ensures only valid JWTs can access this endpoint\",\"user\":\"username\"}"
    );
}

#[tokio::test]
async fn axum_hello_world() {
    cargo_shuttle_run("../examples/axum/hello-world").await;

    let request_text = reqwest::Client::new()
        .get("http://localhost:8000/hello")
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();

    assert_eq!(request_text, "Hello, world!");
}

#[tokio::test]
async fn tide_hello_world() {
    cargo_shuttle_run("../examples/tide/hello-world").await;

    let request_text = reqwest::Client::new()
        .get("http://localhost:8000/hello")
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();

    assert_eq!(request_text, "Hello, world!");
}
