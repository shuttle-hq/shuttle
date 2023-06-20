use std::time::Duration;

use clap::Parser;
use shuttle_common::backends::{
    auth::JwtAuthenticationLayer,
    tracing::{setup_tracing, ExtractPropagationLayer},
};
use shuttle_proto::{
    auth::AuthPublicKey, resource_recorder::resource_recorder_server::ResourceRecorderServer,
};
use shuttle_resource_recorder::{args::Args, Service, Sqlite};
use tonic::transport::Server;
use tracing::trace;

#[tokio::main]
async fn main() {
    let args = Args::parse();

    setup_tracing(tracing_subscriber::registry(), "resource-recorder");

    trace!(args = ?args, "parsed args");

    let db_path = args.state.join("resource-recorder.sqlite");
    let auth_client = shuttle_proto::auth::client(&args.auth_uri)
        .await
        .expect("auth service should be reachable");

    let mut server_builder = Server::builder()
        .http2_keepalive_interval(Some(Duration::from_secs(60)))
        .layer(JwtAuthenticationLayer::new(AuthPublicKey::new(auth_client)))
        .layer(ExtractPropagationLayer);

    let svc = Service::new(Sqlite::new(&db_path.display().to_string()).await);
    let svc = ResourceRecorderServer::new(svc);
    let router = server_builder.add_service(svc);

    router.serve(args.address).await.unwrap();
}
