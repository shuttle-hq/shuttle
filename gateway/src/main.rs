use bollard::Docker;
use clap::Parser;
use futures::prelude::*;
use shuttle_gateway::args::{Args, Commands, InitArgs, ExecCmd};
use shuttle_gateway::auth::Key;
use shuttle_gateway::proxy::make_proxy;
use shuttle_gateway::service::{GatewayService, MIGRATIONS};
use shuttle_gateway::worker::{Work, Worker};
use shuttle_gateway::{api::make_api, args::StartArgs};
use shuttle_gateway::{Refresh, Service, project};
use sqlx::migrate::MigrateDatabase;
use sqlx::{query, Sqlite, SqlitePool};
use std::io;
use std::path::Path;
use std::sync::Arc;
use tracing::{debug, error, info, trace};
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

#[tokio::main(flavor = "multi_thread")]
async fn main() -> io::Result<()> {
    let args = Args::parse();

    trace!(args = ?args, "parsed args");

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
        Commands::Exec(exec_cmd) => exec(db, exec_cmd).await,
    }
}

async fn start(db: SqlitePool, args: StartArgs) -> io::Result<()> {
    let fqdn = args
        .proxy_fqdn
        .to_string()
        .trim_end_matches('.')
        .to_string();
    let gateway = Arc::new(GatewayService::init(args.clone(), fqdn.clone(), db).await);

    let worker = Worker::new(Arc::clone(&gateway));

    let sender = worker.sender();

    let gateway_clone = gateway.clone();
    let sender_clone = sender.clone();

    tokio::spawn(async move {
        for Work {
            project_name,
            account_name,
            work,
        } in gateway_clone
            .iter_projects()
            .await
            .expect("could not list projects")
        {
            match work.refresh(&gateway_clone.context()).await {
                Ok(work) => sender_clone
                    .send(Work {
                        account_name,
                        project_name,
                        work,
                    })
                    .await
                    .unwrap(),
                Err(err) => {
                    error!(
                        error = %err,
                        %account_name,
                        %project_name,
                        "could not refresh state. Skipping it for now.",
                    );
                }
            }
        }
    });

    let worker_handle = tokio::spawn(
        worker
            .start()
            .map_ok(|_| info!("worker terminated successfully"))
            .map_err(|err| error!("worker error: {}", err)),
    );

    let api = make_api(Arc::clone(&gateway), sender);

    let api_handle = tokio::spawn(axum::Server::bind(&args.control).serve(api.into_make_service()));

    let proxy = make_proxy(gateway, fqdn);

    let proxy_handle = tokio::spawn(hyper::Server::bind(&args.user).serve(proxy));

    debug!("starting up all services");

    tokio::select!(
        _ = worker_handle => info!("worker handle finished"),
        _ = api_handle => info!("api handle finished"),
        _ = proxy_handle => info!("proxy handle finished"),
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

async fn exec(db: SqlitePool, exec_cmd: ExecCmd) -> io::Result<()> {
    let docker = Docker::connect_with_local_defaults().unwrap();

    match exec_cmd {
        ExecCmd::Revive => project::exec::revive(db, docker).await
    };

    Ok(())
}
