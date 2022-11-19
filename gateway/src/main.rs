use clap::Parser;
use fqdn::FQDN;
use futures::prelude::*;
use instant_acme::{AccountCredentials, ChallengeType};
use opentelemetry::global;
use shuttle_gateway::acme::{AcmeClient, CustomDomain};
use shuttle_gateway::api::latest::ApiBuilder;
use shuttle_gateway::args::StartArgs;
use shuttle_gateway::args::{Args, Commands, InitArgs, UseTls};
use shuttle_gateway::auth::Key;
use shuttle_gateway::proxy::UserServiceBuilder;
use shuttle_gateway::service::{GatewayService, MIGRATIONS};
use shuttle_gateway::task;
use shuttle_gateway::tls::{make_tls_acceptor, ChainAndPrivateKey};
use shuttle_gateway::worker::Worker;
use sqlx::migrate::MigrateDatabase;
use sqlx::{query, Sqlite, SqlitePool};
use std::io::{self, Cursor};
use std::path::{Path, PathBuf};
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

    let db_path = args.state.join("gateway.sqlite");
    let db_uri = db_path.to_str().unwrap();

    if !db_path.exists() {
        Sqlite::create_database(db_uri).await.unwrap();
    }

    info!(
        "state db: {}",
        std::fs::canonicalize(&args.state)
            .unwrap()
            .to_string_lossy()
    );
    let db = SqlitePool::connect(db_uri).await.unwrap();

    MIGRATIONS.run(&db).await.unwrap();

    match args.command {
        Commands::Start(start_args) => start(db, args.state, start_args).await,
        Commands::Init(init_args) => init(db, init_args).await,
    }
}

async fn start(db: SqlitePool, fs: PathBuf, args: StartArgs) -> io::Result<()> {
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

    let mut api_builder = ApiBuilder::new()
        .with_service(Arc::clone(&gateway))
        .with_sender(sender)
        .binding_to(args.control);

    let mut user_builder = UserServiceBuilder::new()
        .with_service(Arc::clone(&gateway))
        .with_public(args.context.proxy_fqdn.clone())
        .with_user_proxy_binding_to(args.user)
        .with_bouncer(args.bouncer);

    if let UseTls::Enable = args.use_tls {
        let (resolver, tls_acceptor) = make_tls_acceptor();

        user_builder = user_builder
            .with_acme(acme_client.clone())
            .with_tls(tls_acceptor);

        api_builder = api_builder.with_acme(acme_client.clone(), resolver.clone());

        for CustomDomain {
            fqdn,
            certificate,
            private_key,
            ..
        } in gateway.iter_custom_domains().await.unwrap()
        {
            let mut buf = Vec::new();
            buf.extend(certificate);
            buf.extend(private_key);
            resolver
                .serve_pem(&fqdn.to_string(), Cursor::new(buf))
                .await
                .unwrap();
        }

        tokio::spawn(async move {
            // make sure we have a certificate for ourselves
            let certs = init_certs(fs, args.context.proxy_fqdn.clone(), acme_client.clone()).await;
            resolver.serve_default_der(certs).await.unwrap();
        });
    } else {
        warn!("TLS is disabled in the proxy service. This is only acceptable in testing, and should *never* be used in deployments.");
    };

    let api_handle = api_builder
        .with_default_routes()
        .with_default_traces()
        .serve();

    let user_handle = user_builder.serve();

    debug!("starting up all services");

    tokio::select!(
        _ = worker_handle => info!("worker handle finished"),
        _ = api_handle => error!("api handle finished"),
        _ = user_handle => error!("user handle finished"),
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

async fn init_certs<P: AsRef<Path>>(fs: P, public: FQDN, acme: AcmeClient) -> ChainAndPrivateKey {
    let tls_path = fs.as_ref().join("ssl.pem");

    match ChainAndPrivateKey::load_pem(&tls_path) {
        Ok(valid) => valid,
        Err(_) => {
            let creds_path = fs.as_ref().join("acme.json");
            warn!(
                "no valid certificate found at {}, creating one...",
                tls_path.display()
            );

            if !creds_path.exists() {
                panic!(
                    "no ACME credentials found at {}, cannot continue with certificate creation",
                    creds_path.display()
                );
            }

            let creds = std::fs::File::open(creds_path).unwrap();
            let creds: AccountCredentials = serde_json::from_reader(&creds).unwrap();

            let identifier = format!("*.{public}");

            // Use ::Dns01 challenge because that's the only supported
            // challenge type for wildcard domains
            let (chain, private_key) = acme
                .create_certificate(&identifier, ChallengeType::Dns01, creds)
                .await
                .unwrap();

            let mut buf = Vec::new();
            buf.extend(chain.as_bytes());
            buf.extend(private_key.as_bytes());

            let certs = ChainAndPrivateKey::parse_pem(Cursor::new(buf)).unwrap();

            certs.clone().save_pem(&tls_path).unwrap();

            certs
        }
    }
}
