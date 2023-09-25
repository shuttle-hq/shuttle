use anyhow::{bail, Result};
use cargo_shuttle::{CommandOutcome, Shuttle};

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let outcome = Shuttle::new()?.parse_args_and_run().await?;

    if outcome == CommandOutcome::DeploymentFailure {
        // Deployment failure results in a shell error exit code being returned (this allows
        // chaining of commands with `&&` for example to fail at the first deployment failure).
        bail!("");
    }

    Ok(())
}
