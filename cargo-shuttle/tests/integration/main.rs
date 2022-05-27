use std::path::Path;

use cargo_shuttle::{Args, Command, ProjectArgs, Shuttle};
use std::future::Future;

/// creates a `cargo-shuttle` Command instance with some reasonable defaults set.
fn cargo_shuttle_command(
    cmd: Command,
    working_directory: &str,
) -> impl Future<Output = anyhow::Result<()>> {
    let working_directory = Path::new(working_directory).to_path_buf();

    Shuttle::new().run(Args {
        api_url: Some("network support is intentionally broken in tests".to_string()),
        project_args: ProjectArgs {
            working_directory,
            name: None,
        },
        cmd,
    })
}

#[tokio::test]
#[should_panic(expected = "builder error: relative URL without a base")]
async fn network_support_is_intentionally_broken_in_tests() {
    cargo_shuttle_command(Command::Status, ".").await.unwrap();
}

#[tokio::test]
#[should_panic(expected = "No such file or directory")]
async fn fails_if_working_directory_does_not_exist() {
    cargo_shuttle_command(Command::Status, "/path_that_does_not_exist")
        .await
        .unwrap();
}

#[tokio::test]
#[should_panic(expected = "error: could not find `Cargo.toml` in `/` or any parent directory")]
async fn fails_if_working_directory_not_part_of_cargo_workspace() {
    cargo_shuttle_command(Command::Status, "/").await.unwrap();
}
