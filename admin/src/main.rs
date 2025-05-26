use shuttle_admin::{args::Args, run};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let mut args: Args = clap::Parser::parse();
    // don't use an override if production is targetted
    if args
        .api_env
        .as_ref()
        .is_some_and(|e| e == "prod" || e == "production")
    {
        args.api_env = None;
    }

    run(args).await
}
