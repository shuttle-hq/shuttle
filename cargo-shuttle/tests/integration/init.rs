use std::fs::read_to_string;
use std::path::Path;
use std::process::Command;

use cargo_shuttle::{Args, Shuttle};
use clap::Parser;
use indoc::indoc;
use tempfile::Builder;

#[tokio::test]
async fn non_interactive_basic_init() {
    let temp_dir = Builder::new().prefix("basic-init").tempdir().unwrap();
    let temp_dir_path = temp_dir.path().to_owned();

    let args = Args::parse_from([
        "cargo-shuttle",
        "--api-url",
        "http://shuttle.invalid:80",
        "init",
        "--api-key",
        "fake-api-key",
        "--name",
        "my-project",
        "--no-framework",
        temp_dir_path.to_str().unwrap(),
    ]);
    Shuttle::new().unwrap().run(args).await.unwrap();

    let cargo_toml = read_to_string(temp_dir_path.join("Cargo.toml")).unwrap();
    // Expected: name = "basic-initRANDOM_CHARS"
    assert!(cargo_toml.contains("name = \"basic-init"));
    assert!(cargo_toml.contains("shuttle-service = { version = "));
}

#[tokio::test]
async fn non_interactive_rocket_init() {
    let temp_dir = Builder::new().prefix("rocket-init").tempdir().unwrap();
    let temp_dir_path = temp_dir.path().to_owned();

    let args = Args::parse_from([
        "cargo-shuttle",
        "--api-url",
        "http://shuttle.invalid:80",
        "init",
        "--api-key",
        "fake-api-key",
        "--name",
        "my-project",
        "--rocket",
        temp_dir_path.to_str().unwrap(),
    ]);
    Shuttle::new().unwrap().run(args).await.unwrap();

    assert_valid_rocket_project(temp_dir_path.as_path(), "rocket-init");
}

#[test]
fn interactive_rocket_init() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = Builder::new().prefix("rocket-init").tempdir().unwrap();
    let temp_dir_path = temp_dir.path().to_owned();

    let bin_path = assert_cmd::cargo::cargo_bin("cargo-shuttle");
    let mut command = Command::new(bin_path);
    command.args([
        "--api-url",
        "http://shuttle.invalid:80",
        "init",
        "--api-key",
        "fake-api-key",
    ]);
    let mut session = rexpect::session::spawn_command(command, Some(2000))?;

    session.exp_string(
        "How do you want to name your project? It will be hosted at ${project_name}.shuttleapp.rs.",
    )?;
    session.exp_string("Project name")?;
    session.send_line("my-project")?;
    session.exp_string("Where should we create this project?")?;
    session.exp_string("Directory")?;
    session.send_line(temp_dir_path.to_str().unwrap())?;
    session.exp_string(
        "Shuttle works with a range of web frameworks. Which one do you want to use?",
    )?;
    // Partial input should be enough to match "rocket"
    session.send_line("roc")?;
    session.exp_string("Do you want to create the project environment on Shuttle?")?;
    session.send("y")?;
    session.flush()?;
    session.exp_string("yes")?;

    assert_valid_rocket_project(temp_dir_path.as_path(), "rocket-init");

    Ok(())
}

#[test]
fn interactive_rocket_init_dont_prompt_framework() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = Builder::new().prefix("rocket-init").tempdir().unwrap();
    let temp_dir_path = temp_dir.path().to_owned();

    let bin_path = assert_cmd::cargo::cargo_bin("cargo-shuttle");
    let mut command = Command::new(bin_path);
    command.args([
        "--api-url",
        "http://shuttle.invalid:80",
        "init",
        "--api-key",
        "fake-api-key",
        "--rocket",
    ]);
    let mut session = rexpect::session::spawn_command(command, Some(2000))?;

    session.exp_string(
        "How do you want to name your project? It will be hosted at ${project_name}.shuttleapp.rs.",
    )?;
    session.exp_string("Project name")?;
    session.send_line("my-project")?;
    session.exp_string("Where should we create this project?")?;
    session.exp_string("Directory")?;
    session.send_line(temp_dir_path.to_str().unwrap())?;
    session.exp_string("Do you want to create the project environment on Shuttle?")?;
    session.send("y")?;
    session.flush()?;
    session.exp_string("yes")?;

    assert_valid_rocket_project(temp_dir_path.as_path(), "rocket-init");

    Ok(())
}

#[test]
fn interactive_rocket_init_dont_prompt_name() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = Builder::new().prefix("rocket-init").tempdir().unwrap();
    let temp_dir_path = temp_dir.path().to_owned();

    let bin_path = assert_cmd::cargo::cargo_bin("cargo-shuttle");
    let mut command = Command::new(bin_path);
    command.args([
        "--api-url",
        "http://shuttle.invalid:80",
        "init",
        "--api-key",
        "fake-api-key",
        "--name",
        "my-project",
    ]);
    let mut session = rexpect::session::spawn_command(command, Some(2000))?;

    session.exp_string("Where should we create this project?")?;
    session.exp_string("Directory")?;
    session.send_line(temp_dir_path.to_str().unwrap())?;
    session.exp_string(
        "Shuttle works with a range of web frameworks. Which one do you want to use?",
    )?;
    // Partial input should be enough to match "rocket"
    session.send_line("roc")?;
    session.exp_string("Do you want to create the project environment on Shuttle?")?;
    session.send("y")?;
    session.flush()?;
    session.exp_string("yes")?;

    assert_valid_rocket_project(temp_dir_path.as_path(), "rocket-init");

    Ok(())
}

fn assert_valid_rocket_project(path: &Path, name_prefix: &str) {
    let cargo_toml = read_to_string(path.join("Cargo.toml")).unwrap();
    assert!(cargo_toml.contains(&format!("name = \"{name_prefix}")));
    assert!(cargo_toml.contains("shuttle-service = { version = "));
    assert!(cargo_toml.contains("features = [\"web-rocket\"]"));
    assert!(cargo_toml.contains("rocket = "));

    let lib_file = read_to_string(path.join("src").join("lib.rs")).unwrap();
    let expected = indoc! {r#"
    #[macro_use]
    extern crate rocket;

    #[get("/")]
    fn index() -> &'static str {
        "Hello, world!"
    }

    #[shuttle_service::main]
    async fn rocket() -> shuttle_service::ShuttleRocket {
        let rocket = rocket::build().mount("/hello", routes![index]);

        Ok(rocket)
    }"#};

    assert_eq!(lib_file, expected);
}
