use std::{
    fs::read_to_string,
    path::{Path, PathBuf},
};

use cargo_shuttle::{Args, Command, CommandOutcome, InitArgs, ProjectArgs, Shuttle};
use indoc::indoc;
use tempfile::Builder;

/// creates a `cargo-shuttle` init instance with some reasonable defaults set.
async fn cargo_shuttle_init(path: PathBuf) -> anyhow::Result<CommandOutcome> {
    let working_directory = Path::new(".").to_path_buf();

    Shuttle::new()
        .run(Args {
            api_url: Some("http://shuttle.invalid:80".to_string()),
            project_args: ProjectArgs {
                working_directory,
                name: None,
            },
            cmd: Command::Init(InitArgs {
                axum: false,
                rocket: false,
                tide: false,
                tower: false,
                poem: false,
                salvo: false,
                serenity: false,
                path,
            }),
        })
        .await
}

/// creates a `cargo-shuttle` init instance for initializing the `rocket` framework
async fn cargo_shuttle_init_framework(path: PathBuf) -> anyhow::Result<CommandOutcome> {
    let working_directory = Path::new(".").to_path_buf();

    Shuttle::new()
        .run(Args {
            api_url: Some("http://shuttle.invalid:80".to_string()),
            project_args: ProjectArgs {
                working_directory,
                name: None,
            },
            cmd: Command::Init(InitArgs {
                axum: false,
                rocket: true,
                tide: false,
                tower: false,
                poem: false,
                salvo: false,
                serenity: false,
                path,
            }),
        })
        .await
}

#[tokio::test]
async fn basic_init() {
    let temp_dir = Builder::new().prefix("basic-init").tempdir().unwrap();
    let temp_dir_path = temp_dir.path().to_owned();

    cargo_shuttle_init(temp_dir_path.clone()).await.unwrap();
    let cargo_toml = read_to_string(temp_dir_path.join("Cargo.toml")).unwrap();

    // Expected: name = "basic-initRANDOM_CHARS"
    assert!(cargo_toml.contains("name = \"basic-init"));
    assert!(cargo_toml.contains("shuttle-service = { version = "));
}

#[tokio::test]
async fn framework_init() {
    let temp_dir = Builder::new().prefix("rocket-init").tempdir().unwrap();
    let temp_dir_path = temp_dir.path().to_owned();

    cargo_shuttle_init_framework(temp_dir_path.clone())
        .await
        .unwrap();

    let cargo_toml = read_to_string(temp_dir_path.join("Cargo.toml")).unwrap();

    // Expected: name = "rocket-initRANDOM_CHARS"
    assert!(cargo_toml.contains("name = \"rocket-init"));
    assert!(cargo_toml.contains("shuttle-service = { version = "));
    assert!(cargo_toml.contains("features = [\"web-rocket\"]"));
    assert!(cargo_toml.contains("rocket = "));

    let lib_file = read_to_string(temp_dir_path.join("src").join("lib.rs")).unwrap();
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
