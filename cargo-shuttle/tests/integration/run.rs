use cargo_shuttle::{Args, Command, ProjectArgs, RunArgs, Shuttle};
use local_ip_address::local_ip;
use portpicker::pick_unused_port;
use reqwest::StatusCode;
use std::{fs::canonicalize, process::exit, time::Duration};
use tokio::time::sleep;

/// creates a `cargo-shuttle` run instance with some reasonable defaults set.
async fn cargo_shuttle_run(working_directory: &str) -> u16 {
    let working_directory = canonicalize(working_directory).unwrap();
    let port = pick_unused_port().unwrap();
    let run_args = RunArgs { port, router_ip: false};

    let runner = Shuttle::new().unwrap().run(Args {
        api_url: Some("http://shuttle.invalid:80".to_string()),
        project_args: ProjectArgs {
            working_directory: working_directory.clone(),
            name: None,
        },
        cmd: Command::Run(run_args),
    });

    let working_directory_clone = working_directory.clone();

    tokio::spawn(async move {
        sleep(Duration::from_secs(600)).await;

        println!(
            "run test for '{}' took too long. Did it fail to shutdown?",
            working_directory_clone.display()
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
        println!(
            "waiting for '{}' to start up...",
            working_directory.display()
        );
        sleep(Duration::from_millis(350)).await;
    }

    port
}

#[tokio::test(flavor = "multi_thread")]
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

#[tokio::test(flavor = "multi_thread")]
async fn rocket_secrets() {
    let port = cargo_shuttle_run("../examples/rocket/secrets").await;

    let request_text = reqwest::Client::new()
        .get(format!("http://localhost:{port}/secret"))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();

    assert_eq!(request_text, "the contents of my API key");
}

// This example uses a shared Postgres. Thus local runs should create a docker container for it.
#[tokio::test(flavor = "multi_thread")]
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

#[tokio::test(flavor = "multi_thread")]
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

#[tokio::test(flavor = "multi_thread")]
async fn actix_web_hello_world() {
    let port = cargo_shuttle_run("../examples/actix-web/hello-world").await;

    let request_text = reqwest::Client::new()
        .get(format!("http://localhost:{port}/hello"))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();

    assert_eq!(request_text, "Hello World!");
}

#[tokio::test(flavor = "multi_thread")]
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

#[tokio::test(flavor = "multi_thread")]
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

#[tokio::test(flavor = "multi_thread")]
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

#[tokio::test(flavor = "multi_thread")]
async fn warp_hello_world() {
    let port = cargo_shuttle_run("../examples/warp/hello-world").await;

    let request_text = reqwest::Client::new()
        .get(format!("http://localhost:{port}/hello"))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();

    assert_eq!(request_text, "Hello, World!");
}

#[tokio::test(flavor = "multi_thread")]
async fn poem_hello_world() {
    let port = cargo_shuttle_run("../examples/poem/hello-world").await;

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
#[tokio::test(flavor = "multi_thread")]
async fn poem_postgres() {
    let port = cargo_shuttle_run("../examples/poem/postgres").await;
    let client = reqwest::Client::new();

    let post_text = client
        .post(format!("http://localhost:{port}/todo"))
        .body("{\"note\": \"Deploy to shuttle\"}")
        .header("content-type", "application/json")
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

// This example uses a shared MongoDb. Thus local runs should create a docker container for it.
#[tokio::test(flavor = "multi_thread")]
async fn poem_mongodb() {
    let port = cargo_shuttle_run("../examples/poem/mongodb").await;
    let client = reqwest::Client::new();

    // Post a todo note and get the persisted todo objectId
    let post_text = client
        .post(format!("http://localhost:{port}/todo"))
        .body("{\"note\": \"Deploy to shuttle\"}")
        .header("content-type", "application/json")
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();

    // Valid objectId is 24 char hex string
    assert_eq!(post_text.len(), 24);

    let request_text = client
        .get(format!("http://localhost:{port}/todo/{post_text}"))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();

    assert_eq!(request_text, "{\"note\":\"Deploy to shuttle\"}");
}

#[tokio::test(flavor = "multi_thread")]
async fn salvo_hello_world() {
    let port = cargo_shuttle_run("../examples/salvo/hello-world").await;

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

#[tokio::test(flavor = "multi_thread")]
async fn thruster_hello_world() {
    let port = cargo_shuttle_run("../examples/thruster/hello-world").await;

    let request_text = reqwest::Client::new()
        .get(format!("http://localhost:{port}/hello"))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();

    assert_eq!(request_text, "Hello, World!");
}

/// creates a `cargo-shuttle` run instance with some reasonable defaults set, but using the router IP instead.
async fn cargo_shuttle_run_with_router_ip(working_directory: &str) -> u16 {
    let working_directory = canonicalize(working_directory).unwrap();
    let port = pick_unused_port().unwrap();
    let ip = local_ip().unwrap();
    let run_args = RunArgs { port, router_ip: true };

    let runner = Shuttle::new().unwrap().run(Args {
        api_url: Some("http://shuttle.invalid:80".to_string()),
        project_args: ProjectArgs {
            working_directory: working_directory.clone(),
            name: None,
        },
        cmd: Command::Run(run_args),
    });

    let working_directory_clone = working_directory.clone();

    tokio::spawn(async move {
        sleep(Duration::from_secs(600)).await;

        println!(
            "run test for '{}' took too long. Did it fail to shutdown?",
            working_directory_clone.display()
        );
        exit(1);
    });

    tokio::spawn(runner);

    // Wait for service to be responsive
    while (reqwest::Client::new()
        .get(format!("http://{ip}:{port}"))
        .send()
        .await)
        .is_err()
    {
        println!(
            "waiting for '{}' to start up...",
            working_directory.display()
        );
        sleep(Duration::from_millis(350)).await;
    }

    port
}

#[tokio::test(flavor = "multi_thread")]
async fn rocket_hello_world_with_router_ip() {

    let port = cargo_shuttle_run_with_router_ip("../examples/rocket/hello-world").await;
    let ip = local_ip().unwrap();

        let request_text = reqwest::Client::new()
        .get(format!("http://{ip:?}:{port}/hello"))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();

    assert_eq!(request_text, "Hello, world!");
}