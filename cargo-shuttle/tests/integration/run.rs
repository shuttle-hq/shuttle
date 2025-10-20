use std::{process::exit, time::Duration};

use portpicker::pick_unused_port;
use tokio::time::sleep;

/// Runs `shuttle run` in specified directory
async fn shuttle_run(working_directory: &str, external: bool) -> String {
    let port = pick_unused_port().unwrap();

    let url = if !external {
        format!("http://localhost:{port}")
    } else {
        format!("http://0.0.0.0:{port}")
    };

    let bin_path = assert_cmd::cargo::cargo_bin("shuttle");
    let mut command = std::process::Command::new(bin_path);
    command.args([
        "shuttle",
        "--wd",
        working_directory,
        "--offline",
        "run",
        "--port",
        &port.to_string(),
    ]);
    if external {
        command.arg("--external");
    }
    let mut runner = command.spawn().unwrap();

    tokio::spawn({
        let working_directory = working_directory.to_owned();
        async move {
            sleep(Duration::from_secs(10 * 60)).await;

            println!(
                "run test for '{}' took too long. Did it fail to shutdown?",
                working_directory
            );
            exit(1);
        }
    });

    // Wait for service to be responsive
    let mut counter = 0;
    let client = reqwest::Client::new();
    while client.get(url.clone()).send().await.is_err() {
        if runner.try_wait().unwrap().is_some() {
            println!(
                "run test for '{}' exited early. Did it fail to compile/run?",
                working_directory
            );
            exit(1);
        }

        // reduce spam
        if counter == 0 {
            println!("waiting for '{}' to start up...", working_directory);
        }
        counter = (counter + 1) % 10;

        sleep(Duration::from_millis(500)).await;
    }

    url
}

#[tokio::test(flavor = "multi_thread")]
async fn axum_hello_world() {
    let url = shuttle_run("../examples/axum/hello-world", false).await;

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

    let url = shuttle_run("../examples/rocket/secrets", false).await;

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

// These examples use a shared Postgres. Thus local runs should create a docker containers.
mod needs_docker {
    use super::*;

    #[tokio::test(flavor = "multi_thread")]
    async fn rocket_postgres() {
        let url = shuttle_run("../examples/rocket/postgres", false).await;
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
