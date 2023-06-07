use std::time::Duration;

use clap::Parser;
use shuttle_common::backends::tracing::setup_tracing;
use shuttle_proto::auth::auth_server::AuthServer;
use sqlx::migrate::Migrator;
use tonic::transport::Server;
use tracing::trace;

use shuttle_auth::{Args, Commands, EdDsaManager, Service, Sqlite};

pub static MIGRATIONS: Migrator = sqlx::migrate!("./migrations");

#[tokio::main]
async fn main() {
    let args = Args::parse();

    trace!(args = ?args, "parsed args");

    setup_tracing(tracing_subscriber::registry(), "auth");

    let sqlite = Sqlite::new(&args.state.display().to_string()).await;

    match args.command {
        Commands::Start(args) => {
            let mut server_builder =
                Server::builder().http2_keepalive_interval(Some(Duration::from_secs(60)));

            let key_manager = EdDsaManager::default();

            let svc = Service::new(sqlite, key_manager);
            let svc = AuthServer::new(svc);
            let router = server_builder.add_service(svc);

            router.serve(args.address).await.unwrap();
        }
        Commands::Init(args) => sqlite.insert_admin(&args.name, args.key.as_deref()).await,
    }
}
