use std::path::Path;

use cargo_shuttle::{Args, Command, CommandOutcome, DeployArgs, ProjectArgs, Shuttle};
use reqwest::StatusCode;
use test_context::test_context;
use tokiotest_httpserver::{handler::HandlerBuilder, HttpTestContext};

/// creates a `cargo-shuttle` deploy instance with some reasonable defaults set.
async fn cargo_shuttle_deploy(path: &str, api_url: String) -> anyhow::Result<CommandOutcome> {
    let working_directory = Path::new(path).to_path_buf();

    Shuttle::new()
        .run(Args {
            api_url: Some(api_url),
            project_args: ProjectArgs {
                working_directory,
                name: None,
            },
            cmd: Command::Deploy(DeployArgs {
                allow_dirty: false,
                no_test: false,
            }),
        })
        .await
}

#[should_panic(expected = "not an absolute path")]
#[test_context(HttpTestContext)]
#[tokio::test]
async fn deploy_when_version_is_valid(ctx: &mut HttpTestContext) {
    ctx.add(
        HandlerBuilder::new("/test/version")
            .status_code(StatusCode::OK)
            .response(shuttle_service::VERSION.into())
            .build(),
    );
    let api_url = ctx.uri("/test").to_string();

    cargo_shuttle_deploy("../examples/rocket/hello-world", api_url)
        .await
        .unwrap();
}
