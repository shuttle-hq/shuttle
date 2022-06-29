use cargo_shuttle::{Args, Command, ProjectArgs, RunArgs, Shuttle};
use portpicker::pick_unused_port;
use reqwest::StatusCode;
use std::{fs::canonicalize, process::exit, time::Duration};
use tokio::time::sleep;

/// creates a `cargo-shuttle` run instance with some reasonable defaults set.
async fn cargo_shuttle_run(working_directory: &str) -> u16 {
    let _ = env_logger::builder()
        .filter_module("cargo_shuttle", log::LevelFilter::Trace)
        .is_test(true)
        .try_init();
    let working_directory = canonicalize(working_directory).unwrap();
    let port = pick_unused_port().unwrap();
    let run_args = RunArgs { port };

    let runner = Shuttle::new().run(Args {
        api_url: Some("http://shuttle.invalid:80".to_string()),
        project_args: ProjectArgs {
            working_directory: working_directory.clone(),
            name: None,
        },
        cmd: Command::Run(run_args),
    });

    tokio::spawn(async move {
        sleep(Duration::from_secs(180)).await;

        println!(
            "run test for '{}' took too long. Did it fail to shutdown?",
            working_directory.display()
        );
        exit(1);
    });

    tokio::spawn(runner);

    // Wait for service to be responsive
    while (reqwest::Client::new()
        .get(format!("http://localhost:{port}"))
        .send()
        .await)
        .is_err()
    {
        sleep(Duration::from_millis(350)).await;
    }

    port
}

#[tokio::test]
async fn rocket_hello_world() {
    let port = cargo_shuttle_run("../examples/rocket/hello-world").await;

    let request_text = reqwest::Client::new()
        .get(format!("http://localhost:{port}/hello"))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();

    assert_eq!(request_text, "Hello, world!");
}

// This example uses a shared Postgres. Thus local runs should create a docker container for it.
#[tokio::test]
async fn rocket_postgres() {
    let port = cargo_shuttle_run("../examples/rocket/postgres").await;
    let client = reqwest::Client::new();

    let post_text = client
        .post(format!("http://localhost:{port}/todo"))
        .body("{\"note\": \"Deploy to shuttle\"}")
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();

    assert_eq!(post_text, "{\"id\":1,\"note\":\"Deploy to shuttle\"}");

    let request_text = client
        .get(format!("http://localhost:{port}/todo/1"))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();

    assert_eq!(request_text, "{\"id\":1,\"note\":\"Deploy to shuttle\"}");
}

#[tokio::test]
async fn rocket_authentication() {
    let port = cargo_shuttle_run("../examples/rocket/authentication").await;
    let client = reqwest::Client::new();

    let public_text = client
        .get(format!("http://localhost:{port}/public"))
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
        .get(format!("http://localhost:{port}/private"))
        .send()
        .await
        .unwrap()
        .status();

    assert_eq!(private_status, StatusCode::FORBIDDEN);

    let body = client
        .post(format!("http://localhost:{port}/login"))
        .body("{\"username\": \"username\", \"password\": \"password\"}")
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    let token = format!("Bearer  {}", json["token"].as_str().unwrap());

    let private_text = client
        .get(format!("http://localhost:{port}/private"))
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
    let port = cargo_shuttle_run("../examples/axum/hello-world").await;

    let request_text = reqwest::Client::new()
        .get(format!("http://localhost:{port}/hello"))
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
    let port = cargo_shuttle_run("../examples/tide/hello-world").await;

    let request_text = reqwest::Client::new()
        .get(format!("http://localhost:{port}/hello"))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();

    assert_eq!(request_text, "Hello, world!");
}

#[tokio::test]
async fn tower_hello_world() {
    let port = cargo_shuttle_run("../examples/tower/hello-world").await;

    let request_text = reqwest::Client::new()
        .get(format!("http://localhost:{port}/hello"))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();

    assert_eq!(request_text, "Hello, world!");
}
