use std::{fs::canonicalize, process::exit, time::Duration};

use cargo_shuttle::{Command, ProjectArgs, RunArgs, Shuttle, ShuttleArgs};
use portpicker::pick_unused_port;
use tokio::time::sleep;

/// Runs `shuttle run` in specified directory
async fn shuttle_run(working_directory: &str, external: bool) -> String {
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

    let runner = Shuttle::new(cargo_shuttle::Binary::Shuttle, None)
        .unwrap()
        .run(
            ShuttleArgs {
                api_url: Some("http://shuttle.invalid:80".to_string()),
                admin: false,
                api_env: None,
                project_args: ProjectArgs {
                    working_directory: working_directory.clone(),
                    name_or_id: None,
                },
                offline: false,
                debug: false,
                output_mode: Default::default(),
                cmd: Command::Run(RunArgs {
                    port,
                    external,
                    release: false,
                    raw: false,
                    bacon: false,
                    secret_args: Default::default(),
                }),
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
