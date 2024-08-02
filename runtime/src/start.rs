use anyhow::Context;

use crate::{
    __internals::{Loader, Runner},
    alpha, beta, version,
};

#[derive(Default)]
struct Args {
    /// Enable compatibility with beta platform
    beta: bool,
    /// Alpha (required): Port to open gRPC server on
    port: Option<u16>,
    /// Beta (required): Run the app (allows erroring when `cargo run` is used)
    run: bool,
}

impl Args {
    // uses simple arg parsing logic instead of clap to reduce dependency weight
    fn parse() -> anyhow::Result<Self> {
        let mut args = Self::default();

        // The first argument is the path of the executable
        let mut args_iter = std::env::args().skip(1);

        while let Some(arg) = args_iter.next() {
            match arg.as_str() {
                "--port" => {
                    let port = args_iter
                        .next()
                        .context("missing port value")?
                        .parse()
                        .context("invalid port value")?;
                    args.port = Some(port);
                }
                "--run" => {
                    args.run = true;
                }
                _ => {}
            }
        }

        args.beta = std::env::var("SHUTTLE_BETA").is_ok();

        if args.beta {
            if !args.run {
                return Err(anyhow::anyhow!("--run is required with --beta"));
            }
        } else if args.port.is_none() {
            return Err(anyhow::anyhow!("--port is required"));
        }

        Ok(args)
    }
}

pub async fn start(loader: impl Loader + Send + 'static, runner: impl Runner + Send + 'static) {
    // `--version` overrides any other arguments. Used by cargo-shuttle to check compatibility on local runs.
    if std::env::args().any(|arg| arg == "--version") {
        println!("{}", version());
        return;
    }

    let args = match Args::parse() {
        Ok(args) => args,
        Err(e) => {
            eprintln!("Runtime failed to parse args: {e}");
            let help_str = "[HINT]: Run your Shuttle app with `cargo shuttle run`";
            let wrapper_str = "-".repeat(help_str.len());
            eprintln!("{wrapper_str}\n{help_str}\n{wrapper_str}");
            return;
        }
    };

    println!("{} {} executable started", crate::NAME, crate::VERSION);

    // this is handled after arg parsing to not interfere with --version above
    #[cfg(feature = "setup-tracing")]
    {
        use colored::Colorize;
        use tracing_subscriber::prelude::*;

        colored::control::set_override(true); // always apply color

        tracing_subscriber::registry()
            .with(tracing_subscriber::fmt::layer().without_time())
            .with(
                // let user override RUST_LOG in local run if they want to
                tracing_subscriber::EnvFilter::try_from_default_env()
                    // otherwise use our default
                    .or_else(|_| tracing_subscriber::EnvFilter::try_new("info,shuttle=trace"))
                    .unwrap(),
            )
            .init();

        println!(
            "{}",
            "Shuttle's default tracing subscriber is initialized!".yellow(),
        );
        println!("To disable it and use your own, check the docs: https://docs.shuttle.rs/configuration/logs");
    }

    if args.beta {
        beta::start(loader, runner).await
    } else {
        alpha::start(args.port.unwrap(), loader, runner).await
    }
}
