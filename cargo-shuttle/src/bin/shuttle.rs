use anyhow::Result;
use cargo_shuttle::{parse_args, setup_tracing, Binary, Shuttle};

#[tokio::main]
async fn main() -> Result<()> {
    let (args, provided_path_to_init) = parse_args();

    setup_tracing(args.debug);

    Shuttle::new(Binary::Shuttle, args.api_env.clone())?
        .run(args, provided_path_to_init)
        .await
        .map(|_| ())
}
