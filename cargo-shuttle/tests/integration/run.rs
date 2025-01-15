use std::{fs::canonicalize, process::exit, time::Duration};

use cargo_shuttle::{Command, ProjectArgs, RunArgs, Shuttle, ShuttleArgs};
use portpicker::pick_unused_port;
use reqwest::StatusCode;
use tokio::time::sleep;

/// creates a `cargo-shuttle` run instance with some reasonable defaults set.
pub async fn cargo_shuttle_run(working_directory: &str, external: bool) -> String {
    let working_directory = match canonicalize(working_directory) {
        Ok(wd) => wd,
        Err(e) => {
            // DEBUG CI (no such file): SLEEP AND TRY AGAIN?
            println!(
                "Did not find directory: {} !!! because {:?}",
                working_directory, e
            );
            sleep(Duration::from_millis(500)).await;
            canonicalize(working_directory).unwrap()
        }
    };

    let port = pick_unused_port().unwrap();

    let url = if !external {
        format!("http://localhost:{port}")
    } else {
        format!("http://0.0.0.0:{port}")
    };

    let run_args = RunArgs {
        port,
        external,
        release: false,
        raw: false,
        secret_args: Default::default(),
    };

    let runner = Shuttle::new(cargo_shuttle::Binary::CargoShuttle)
        .unwrap()
        .run(
            ShuttleArgs {
                api_url: Some("http://shuttle.invalid:80".to_string()),
                project_args: ProjectArgs {
                    working_directory: working_directory.clone(),
                    name_or_id: None,
                },
                offline: false,
                debug: false,
                beta: false,
                cmd: Command::Run(run_args),
            },
            false,
        );

    tokio::spawn({
        let working_directory = working_directory.clone();
        async move {
            sleep(Duration::from_secs(10 * 60)).await;

            println!(
                "run test for '{}' took too long. Did it fail to shutdown?",
                working_directory.display()
            );
            exit(1);
        }
    });

    let runner_handle = tokio::spawn(runner);

    // Wait for service to be responsive
    let mut counter = 0;
    let client = reqwest::Client::new();
    while client.get(url.clone()).send().await.is_err() {
        if runner_handle.is_finished() {
            println!(
                "run test for '{}' exited early. Did it fail to compile/run?",
                working_directory.clone().display()
            );
            exit(1);
        }

        // reduce spam
        if counter == 0 {
            println!(
                "waiting for '{}' to start up...",
                working_directory.display()
            );
        }
        counter = (counter + 1) % 10;

        sleep(Duration::from_millis(500)).await;
    }

    url
}

#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn rocket_hello_world() {
    let url = cargo_shuttle_run("../examples/rocket/hello-world", false).await;

    let request_text = reqwest::Client::new()
        .get(url)
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();

    assert_eq!(request_text, "Hello, world!");
}

#[tokio::test(flavor = "multi_thread")]
async fn rocket_secrets() {
    std::fs::copy(
        "../examples/rocket/secrets/Secrets.toml.example",
        "../examples/rocket/secrets/Secrets.toml",
    )
    .unwrap();

    let url = cargo_shuttle_run("../examples/rocket/secrets", false).await;

    let request_text = reqwest::Client::new()
        .get(format!("{url}/secret"))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();

    assert_eq!(request_text, "the contents of my API key");
}

#[tokio::test(flavor = "multi_thread")]
async fn axum_static_files() {
    let url = cargo_shuttle_run("../examples/axum/static-files", false).await;
    let client = reqwest::Client::new();

    let request_text = client
        .get(url.clone())
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();

    assert!(request_text.contains("This is an example of serving"));
}

#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn rocket_authentication() {
    let url = cargo_shuttle_run("../examples/rocket/authentication", false).await;
    let client = reqwest::Client::new();

    let public_text = client
        .get(format!("{url}/public"))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();

    assert_eq!(
        public_text,
        r#"{"message":"This endpoint is open to anyone"}"#
    );

    let private_status = client
        .get(format!("{url}/private"))
        .send()
        .await
        .unwrap()
        .status();

    assert_eq!(private_status, StatusCode::FORBIDDEN);

    let body = client
        .post(format!("{url}/login"))
        .body(r#"{"username": "username", "password": "password"}"#)
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    let token = format!("Bearer  {}", json["token"].as_str().unwrap());

    let private_text = client
        .get(format!("{url}/private"))
        .header("Authorization", token)
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();

    assert_eq!(
        private_text,
        r#"{"message":"The `Claims` request guard ensures only valid JWTs can access this endpoint","user":"username"}"#
    );
}

#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn actix_web_hello_world() {
    let url = cargo_shuttle_run("../examples/actix-web/hello-world", false).await;

    let request_text = reqwest::Client::new()
        .get(url)
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();

    assert_eq!(request_text, "Hello World!");
}

#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn axum_hello_world() {
    let url = cargo_shuttle_run("../examples/axum/hello-world", false).await;

    let request_text = reqwest::Client::new()
        .get(url)
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();

    assert_eq!(request_text, "Hello, world!");
}

#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn tide_hello_world() {
    let url = cargo_shuttle_run("../examples/tide/hello-world", false).await;

    let request_text = reqwest::Client::new()
        .get(url)
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();

    assert_eq!(request_text, "Hello, world!");
}

#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn tower_hello_world() {
    let url = cargo_shuttle_run("../examples/tower/hello-world", false).await;

    let request_text = reqwest::Client::new()
        .get(url)
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();

    assert_eq!(request_text, "Hello, world!");
}

#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn warp_hello_world() {
    let url = cargo_shuttle_run("../examples/warp/hello-world", false).await;

    let request_text = reqwest::Client::new()
        .get(url)
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();

    assert_eq!(request_text, "Hello, World!");
}

#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn poem_hello_world() {
    let url = cargo_shuttle_run("../examples/poem/hello-world", false).await;

    let request_text = reqwest::Client::new()
        .get(url)
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();

    assert_eq!(request_text, "Hello, world!");
}

#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn salvo_hello_world() {
    let url = cargo_shuttle_run("../examples/salvo/hello-world", false).await;

    let request_text = reqwest::Client::new()
        .get(url)
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();

    assert_eq!(request_text, "Hello, world!");
}

#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn thruster_hello_world() {
    let url = cargo_shuttle_run("../examples/thruster/hello-world", false).await;

    let request_text = reqwest::Client::new()
        .get(url)
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();

    assert_eq!(request_text, "Hello, World!");
}

#[tokio::test(flavor = "multi_thread")]
async fn rocket_hello_world_with_router_ip() {
    let url = cargo_shuttle_run("../examples/rocket/hello-world", true).await;

    let request_text = reqwest::Client::new()
        .get(url)
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();

    assert_eq!(request_text, "Hello, world!");
}

// These examples use a shared Postgres/Mongo. Thus local runs should create a docker containers.
mod needs_docker {
    use super::*;

    #[tokio::test(flavor = "multi_thread")]
    #[ignore]
    async fn rocket_postgres() {
        let url = cargo_shuttle_run("../examples/rocket/postgres", false).await;
        let client = reqwest::Client::new();

        let post_text = client
            .post(format!("{url}/todo"))
            .body("{\"note\": \"Deploy to shuttle\"}")
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap();

        assert_eq!(post_text, "{\"id\":1,\"note\":\"Deploy to shuttle\"}");

        let request_text = client
            .get(format!("{url}/todo/1"))
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap();

        assert_eq!(request_text, "{\"id\":1,\"note\":\"Deploy to shuttle\"}");
    }
}
