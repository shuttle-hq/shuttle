use crate::{
    __internals::{Loader, Runner},
    rt,
};

/// Uses simple arg parsing logic instead of clap to reduce dependency weight.
/// The rest of the args are parsed in `RuntimeEnvVars`.
fn initial_args_and_env_check() -> anyhow::Result<()> {
    if std::env::args().any(|arg| arg == "--port") {
        anyhow::bail!("Outdated argument detected (--port). Upgrade your Shuttle CLI.");
    }

    if std::env::var("SHUTTLE_ENV").is_err() {
        anyhow::bail!("SHUTTLE_ENV is required to be set on shuttle.dev");
    }

    Ok(())
}

pub async fn start(
    loader: impl Loader + Send + 'static,
    runner: impl Runner + Send + 'static,
    crate_name: &'static str,
    package_version: &'static str,
) {
    // `--version` overrides any other arguments. Used by cargo-shuttle to check compatibility on local runs.
    if std::env::args().any(|arg| arg == "--version") {
        println!("{}", crate::VERSION_STRING);
        return;
    }

    println!(
        "{} starting: {} {}",
        crate::VERSION_STRING,
        crate_name,
        package_version
    );

    if let Err(e) = initial_args_and_env_check() {
        eprintln!("ERROR: Runtime failed to parse args: {e}");
        let help_str = "[HINT]: Run your Shuttle app with `shuttle run`";
        let wrapper_str = "-".repeat(help_str.len());
        eprintln!("{wrapper_str}\n{help_str}\n{wrapper_str}");
        return;
    }

    // this is handled after arg parsing to not interfere with --version above
    #[cfg(all(feature = "setup-tracing", not(feature = "setup-otel-exporter")))]
    {
        use tracing_subscriber::{fmt, prelude::*, registry, EnvFilter};
        registry()
            .with(fmt::layer().without_time())
            .with(
                // let user override RUST_LOG in local run if they want to
                EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                    // otherwise use our default
                    format!("info,{}=debug", crate_name).into()
                }),
            )
            .init();
        tracing::warn!(
            "Default tracing subscriber initialized (https://docs.shuttle.dev/docs/logs)"
        );
    }

    #[cfg(feature = "setup-otel-exporter")]
    let _guard = {
        use tracing_subscriber::{fmt, prelude::*, registry, EnvFilter};
        let (layers, guard) =
            crate::telemetry::otel_tracing_subscriber(crate_name, package_version);

        registry()
            .with(layers)
            .with(fmt::layer().without_time())
            .with(
                // let user override RUST_LOG in local run if they want to
                EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                    // otherwise use our default
                    format!("info,{}=debug", crate_name).into()
                }),
            )
            .init();
        tracing::warn!(
            "Default tracing subscriber with otel exporter initialized (https://docs.shuttle.dev/docs/telemetry)"
        );

        guard
    };

    let exit_code = rt::start(loader, runner).await;

    // TODO: drop/shutdown logger guards

    std::process::exit(exit_code)
}
