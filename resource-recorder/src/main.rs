use std::time::Duration;

use clap::Parser;
use shuttle_common::{
    backends::{
        auth::{AuthPublicKey, JwtAuthenticationLayer},
        client::gateway,
        trace::setup_tracing,
    },
    extract_propagation::ExtractPropagationLayer,
    log::Backend,
};
use shuttle_proto::resource_recorder::resource_recorder_server::ResourceRecorderServer;
use shuttle_resource_recorder::{args::Args, Service, Sqlite};
use tonic::transport::Server;

#[tokio::main]
async fn main() {
    let Args {
        address,
        state,
        auth_uri,
        gateway_uri,
    } = Args::parse();

    setup_tracing(tracing_subscriber::registry(), Backend::ResourceRecorder);

    let mut server_builder = Server::builder()
        .http2_keepalive_interval(Some(Duration::from_secs(60)))
        .layer(JwtAuthenticationLayer::new(AuthPublicKey::new(auth_uri)))
        .layer(ExtractPropagationLayer);

    let gateway_client = gateway::Client::new(gateway_uri.clone(), gateway_uri);

    let db_path = state.join("resource-recorder.sqlite");
    let svc = Service::new(
        Sqlite::new(db_path.display().to_string().as_str()).await,
        gateway_client,
    );
    let svc = ResourceRecorderServer::new(svc);
    let router = server_builder.add_service(svc);

    router.serve(address).await.unwrap();
}
