use std::process::Command;
use std::str;

mod helpers;

#[test]
fn hello_world() {
    let _api = helpers::Api::new();

    let unveil_output = Command::new("cargo")
        .args([
            "run",
            "--bin",
            "cargo-unveil",
            "--manifest-path",
            "../../../Cargo.toml",
            "--",
            "deploy",
        ])
        .current_dir("../examples/rocket/hello-world")
        .output()
        .unwrap();

    let stdout = str::from_utf8(&unveil_output.stdout).unwrap();
    assert!(
        stdout.contains("Finished dev"),
        "output does not contain 'Finished dev':\nstdout = {}\nstderr = {}",
        stdout,
        str::from_utf8(&unveil_output.stderr).unwrap()
    );
    assert!(stdout.contains("Deployment Status:  DEPLOYED"));

    let request_text = reqwest::blocking::Client::new()
        .get("http://localhost:8000/hello")
        .header("Host", "hello-world-rocket-app.unveil.sh")
        .send()
        .unwrap()
        .text()
        .unwrap();

    assert_eq!(request_text, "Hello, world!");
}

#[test]
fn postgres() {
    let api = helpers::Api::new();

    let unveil_output = Command::new("cargo")
        .args([
            "run",
            "--bin",
            "cargo-unveil",
            "--manifest-path",
            "../../../Cargo.toml",
            "--",
            "deploy",
        ])
        .current_dir("../examples/rocket/postgres")
        .output()
        .unwrap();

    let stdout = str::from_utf8(&unveil_output.stdout).unwrap();
    assert!(
        stdout.contains("Finished dev"),
        "output does not contain 'Finished dev':\nstdout = {}\nstderr = {}",
        stdout,
        str::from_utf8(&unveil_output.stderr).unwrap()
    );
    assert!(stdout.contains("Deployment Status:  DEPLOYED"));

    let client = reqwest::blocking::Client::new();
    let add_response = client
        .post("http://localhost:8000/todo")
        .body("{\"note\": \"To the stars\"}")
        .header("Host", "postgres-rocket-app.unveil.sh")
        .send()
        .unwrap()
        .text()
        .unwrap();

    assert_eq!(add_response, "{\"id\":1,\"note\":\"To the stars\"}");

    let fetch_response: String = client
        .get("http://localhost:8000/todo/1")
        .header("Host", "postgres-rocket-app.unveil.sh")
        .send()
        .unwrap()
        .text()
        .unwrap();

    assert_eq!(fetch_response, "{\"id\":1,\"note\":\"To the stars\"}");

    drop(api);
}
