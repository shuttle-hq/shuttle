use std::time::Duration;

use clap::Parser;
use shuttle_common::{
    backends::tracing::{setup_tracing, ExtractPropagationLayer},
    ApiKey,
};
use shuttle_proto::auth::auth_server::AuthServer;
use tonic::transport::Server;
use tracing::trace;

use shuttle_auth::{AccountTier, Args, Commands, Dal, EdDsaManager, Service, Sqlite};

#[tokio::main]
async fn main() {
    let args = Args::parse();

    trace!(args = ?args, "parsed args");

    setup_tracing(tracing_subscriber::registry(), "auth");

    let db_path = args.state.join("auth.sqlite");

    let sqlite = Sqlite::new(db_path.to_str().unwrap()).await;

    match args.command {
        Commands::Start(args) => {
            let mut server_builder = Server::builder()
                .http2_keepalive_interval(Some(Duration::from_secs(60)))
                .layer(ExtractPropagationLayer);

            let key_manager = EdDsaManager::default();

            let svc = Service::new(sqlite, key_manager);
            let svc = AuthServer::new(svc);
            let router = server_builder.add_service(svc);

            router.serve(args.address).await.unwrap();
        }
        Commands::Init(args) => {
            let key = args
                .key
                .map_or_else(ApiKey::generate, |key| ApiKey::parse(&key).unwrap());

            sqlite
                .create_user(args.name.into(), key, AccountTier::Admin)
                .await
                .unwrap();
        }
    }
}
