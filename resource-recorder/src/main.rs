use std::time::Duration;

use clap::Parser;
use shuttle_common::{
    backends::{
        auth::{AuthPublicKey, JwtAuthenticationLayer},
        trace::{setup_tracing, ExtractPropagationLayer},
    },
    log::Backend,
};
use shuttle_proto::resource_recorder::resource_recorder_server::ResourceRecorderServer;
use shuttle_resource_recorder::{args::Args, Service, Sqlite};
use tonic::transport::Server;
use tracing::trace;

#[tokio::main]
async fn main() {
    let args = Args::parse();

    setup_tracing(
        tracing_subscriber::registry(),
        Backend::ResourceRecorder,
        None,
    );

    trace!(args = ?args, "parsed args");

    let mut server_builder = Server::builder()
        .http2_keepalive_interval(Some(Duration::from_secs(60)))
        .layer(JwtAuthenticationLayer::new(AuthPublicKey::new(
            args.auth_uri,
        )))
        .layer(ExtractPropagationLayer);

    let db_path = args.state.join("resource-recorder.sqlite");
    let svc = Service::new(Sqlite::new(db_path.display().to_string().as_str()).await);
    let svc = ResourceRecorderServer::new(svc);
    let router = server_builder.add_service(svc);

    router.serve(args.address).await.unwrap();
}
