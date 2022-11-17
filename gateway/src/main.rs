use clap::Parser;
use futures::prelude::*;
use opentelemetry::global;
use shuttle_gateway::acme::AcmeClient;
use shuttle_gateway::args::{Args, Commands, InitArgs, UseTls};
use shuttle_gateway::auth::Key;
use shuttle_gateway::proxy::make_proxy;
use shuttle_gateway::service::{GatewayService, MIGRATIONS};
use shuttle_gateway::task;
use shuttle_gateway::tls::make_tls_acceptor;
use shuttle_gateway::worker::Worker;
use shuttle_gateway::{api::make_api, args::StartArgs};
use sqlx::migrate::MigrateDatabase;
use sqlx::{query, Sqlite, SqlitePool};
use std::io;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, error, info, trace, warn};
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

#[tokio::main(flavor = "multi_thread")]
async fn main() -> io::Result<()> {
    let args = Args::parse();

    trace!(args = ?args, "parsed args");

    global::set_text_map_propagator(opentelemetry_datadog::DatadogPropagator::new());

    let fmt_layer = fmt::layer();
    let filter_layer = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new("info"))
        .unwrap();

    let tracer = opentelemetry_datadog::new_pipeline()
        .with_service_name("gateway")
        .with_agent_endpoint("http://datadog-agent:8126")
        .install_batch(opentelemetry::runtime::Tokio)
        .unwrap();
    let opentelemetry = tracing_opentelemetry::layer().with_tracer(tracer);
    tracing_subscriber::registry()
        .with(filter_layer)
        .with(fmt_layer)
        .with(opentelemetry)
        .init();

    if !Path::new(&args.state).exists() {
        Sqlite::create_database(&args.state).await.unwrap();
    }

    info!(
        "state db: {}",
        std::fs::canonicalize(&args.state)
            .unwrap()
            .to_string_lossy()
    );
    let db = SqlitePool::connect(&args.state).await.unwrap();

    MIGRATIONS.run(&db).await.unwrap();

    match args.command {
        Commands::Start(start_args) => start(db, start_args).await,
        Commands::Init(init_args) => init(db, init_args).await,
    }
}

async fn start(db: SqlitePool, args: StartArgs) -> io::Result<()> {
    let gateway = Arc::new(GatewayService::init(args.context.clone(), db).await);

    let worker = Worker::new();

    let sender = worker.sender();

    for (project_name, account_name) in gateway
        .iter_projects()
        .await
        .expect("could not list projects")
    {
        gateway
            .clone()
            .new_task()
            .project(project_name)
            .account(account_name)
            .and_then(task::refresh())
            .send(&sender)
            .await
            .ok()
            .unwrap();
    }

    let worker_handle = tokio::spawn(
        worker
            .start()
            .map_ok(|_| info!("worker terminated successfully"))
            .map_err(|err| error!("worker error: {}", err)),
    );

    // Every 60secs go over all `::Ready` projects and check their
    // health
    let ambulance_handle = tokio::spawn({
        let gateway = Arc::clone(&gateway);
        let sender = sender.clone();
        async move {
            loop {
                tokio::time::sleep(Duration::from_secs(60)).await;
                if let Ok(projects) = gateway.iter_projects().await {
                    for (project_name, account_name) in projects {
                        let _ = gateway
                            .new_task()
                            .project(project_name)
                            .account(account_name)
                            .and_then(task::check_health())
                            .send(&sender)
                            .await;
                    }
                }
            }
        }
    });

    let acme_client = AcmeClient::new();

    let api = make_api(Arc::clone(&gateway), acme_client.clone(), sender);

    let api_handle = tokio::spawn(axum::Server::bind(&args.control).serve(api.into_make_service()));

    let (bouncer, user_proxy) =
        make_proxy(Arc::clone(&gateway), acme_client, args.context.proxy_fqdn);
    let proxy_handle = if let UseTls::Disable = args.use_tls {
        warn!("TLS is disabled in the proxy service. This is only acceptable in testing, and should *never* be used in deployments.");
        axum_server::Server::bind(args.user)
            .serve(user_proxy)
            .boxed()
    } else {
        let (resolver, tls_acceptor) = make_tls_acceptor();
        axum_server::Server::bind(args.user)
            .acceptor(tls_acceptor)
            .serve(user_proxy)
            .boxed()
    };

    debug!("starting up all services");

    tokio::select!(
        _ = worker_handle => info!("worker handle finished"),
        _ = api_handle => error!("api handle finished"),
        _ = proxy_handle => error!("proxy handle finished"),
        _ = ambulance_handle => error!("ambulance handle finished"),
    );

    Ok(())
}

async fn init(db: SqlitePool, args: InitArgs) -> io::Result<()> {
    let key = match args.key {
        Some(key) => key,
        None => Key::new_random(),
    };

    query("INSERT INTO accounts (account_name, key, super_user) VALUES (?1, ?2, 1)")
        .bind(&args.name)
        .bind(&key)
        .execute(&db)
        .await
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

    println!("`{}` created as super user with key: {key}", args.name);
    Ok(())
}
