use std::{
    fs::{read_to_string, remove_dir_all},
    path::Path,
};

use cargo_shuttle::{Args, Command, InitArgs, ProjectArgs, Shuttle};
use futures::Future;

/// creates a `cargo-shuttle` init instance with some reasonable defaults set.
fn cargo_shuttle_init(path: &str) -> impl Future<Output = anyhow::Result<()>> {
    let _result = remove_dir_all(path);

    let working_directory = Path::new(".").to_path_buf();
    let path = Path::new(path).to_path_buf();

    Shuttle::new().run(Args {
        api_url: Some("network support is intentionally broken in tests".to_string()),
        project_args: ProjectArgs {
            working_directory,
            name: None,
        },
        cmd: Command::Init(InitArgs { path }),
    })
}

#[tokio::test]
async fn basic_init() {
    cargo_shuttle_init("tests/tmp/basic-init").await.unwrap();

    let cargo_toml = read_to_string("tests/tmp/basic-init/Cargo.toml").unwrap();

    assert!(cargo_toml.contains("name = \"basic-init\""));
    assert!(cargo_toml.contains("shuttle-service = { version = "));
}
