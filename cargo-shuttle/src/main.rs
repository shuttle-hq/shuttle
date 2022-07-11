use anyhow::Result;
use cargo_shuttle::{Args, CommandOutcome, Shuttle};
use structopt::StructOpt;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    let result = Shuttle::new().run(Args::from_args()).await;

    if matches!(result, Ok(CommandOutcome::DeploymentFailure)) {
        // Deployment failure results in a shell error exit code being returned (this allows
        // chaining of commands with `&&` for example to fail at the first deployment failure).
        std::process::exit(1); // TODO: use `std::process::ExitCode::FAILURE` once stable.
    }

    result.map(|_| ())
}
