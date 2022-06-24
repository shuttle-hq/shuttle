use std::path::Path;

use cargo_shuttle::{Args, Command, DeployArgs, ProjectArgs, Shuttle};
use futures::Future;
use reqwest::StatusCode;
use test_context::test_context;
use tokiotest_httpserver::{handler::HandlerBuilder, HttpTestContext};

/// creates a `cargo-shuttle` deploy instance with some reasonable defaults set.
fn cargo_shuttle_deploy(path: &str, api_url: String) -> impl Future<Output = anyhow::Result<()>> {
    let working_directory = Path::new(path).to_path_buf();

    Shuttle::new().run(Args {
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
}

#[should_panic(
    expected = "Your shuttle_service version is outdated. Update your shuttle_service version to 1.2.5 and try to deploy again"
)]
#[test_context(HttpTestContext)]
#[tokio::test]
async fn deploy_when_version_is_outdated(ctx: &mut HttpTestContext) {
    ctx.add(
        HandlerBuilder::new("/test/version")
            .status_code(StatusCode::OK)
            .response("1.2.5".into())
            .build(),
    );
    let api_url = ctx.uri("/test").to_string();

    cargo_shuttle_deploy("../examples/rocket/hello-world", api_url)
        .await
        .unwrap();
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
