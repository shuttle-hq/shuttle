mod init;
mod run;

use cargo_shuttle::{Args, Command, CommandOutcome, ProjectArgs, Shuttle};
use std::path::Path;

/// creates a `cargo-shuttle` run instance with some reasonable defaults set.
async fn cargo_shuttle_command(
    cmd: Command,
    working_directory: &str,
) -> anyhow::Result<CommandOutcome> {
    let working_directory = Path::new(working_directory).to_path_buf();

    Shuttle::new()
        .unwrap()
        .run(Args {
            api_url: Some("http://shuttle.invalid:80".to_string()),
            project_args: ProjectArgs {
                working_directory,
                name: None,
            },
            cmd,
        })
        .await
}

#[tokio::test]
#[should_panic(
    expected = "Could not locate the root of a cargo project. Are you inside a cargo project? You can also use `--working-directory` to locate your cargo project."
)]
async fn fails_if_working_directory_does_not_exist() {
    cargo_shuttle_command(Command::Status, "/path_that_does_not_exist")
        .await
        .unwrap();
}

#[tokio::test]
#[should_panic(
    expected = "Could not locate the root of a cargo project. Are you inside a cargo project? You can also use `--working-directory` to locate your cargo project."
)]
async fn fails_if_working_directory_not_part_of_cargo_workspace() {
    cargo_shuttle_command(Command::Status, "/").await.unwrap();
}
