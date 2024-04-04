use anyhow::Result;
use cargo_shuttle::{parse_args, CommandOutcome, Shuttle};
use tracing_subscriber::{fmt, prelude::*, registry, EnvFilter};

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<()> {
    let (args, provided_path_to_init) = parse_args();

    registry()
        .with(fmt::layer())
        .with(
            // let user set RUST_LOG if they want to
            EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                if args.debug {
                    EnvFilter::new("info,cargo_shuttle=trace,shuttle=trace")
                } else {
                    EnvFilter::default()
                }
            }),
        )
        .init();

    let outcome = Shuttle::new()?.run(args, provided_path_to_init).await?;

    if outcome == CommandOutcome::DeploymentFailure {
        // Deployment failure results in a shell error exit code being returned (this allows
        // chaining of commands with `&&` for example to fail at the first deployment failure).
        std::process::exit(1);
    }

    Ok(())
}
