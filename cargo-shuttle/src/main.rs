use anyhow::Result;
use cargo_shuttle::{CommandOutcome, Shuttle};

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let result = Shuttle::new()?.parse_args_and_run().await;

    if matches!(result, Ok(CommandOutcome::DeploymentFailure)) {
        // Deployment failure results in a shell error exit code being returned (this allows
        // chaining of commands with `&&` for example to fail at the first deployment failure).
        std::process::exit(1); // TODO: use `std::process::ExitCode::FAILURE` once stable.
    }

    result.map(|_| ())
}
