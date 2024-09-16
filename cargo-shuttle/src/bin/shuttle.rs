use anyhow::Result;
use cargo_shuttle::{parse_args, setup_tracing, Binary, CommandOutcome, Shuttle};

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<()> {
    let (args, provided_path_to_init) = parse_args();

    setup_tracing(args.debug);

    let outcome = Shuttle::new(Binary::Shuttle)?
        .run(args, provided_path_to_init)
        .await?;

    if outcome == CommandOutcome::DeploymentFailure {
        // Deployment failure results in a shell error exit code being returned (this allows
        // chaining of commands with `&&` for example to fail at the first deployment failure).
        std::process::exit(1);
    }

    Ok(())
}
