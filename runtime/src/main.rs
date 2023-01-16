use std::{
    net::{Ipv4Addr, SocketAddr},
    time::Duration,
};

use clap::Parser;
use shuttle_common::storage_manager::{ArtifactsStorageManager, WorkingDirStorageManager};
use shuttle_proto::runtime::runtime_server::RuntimeServer;
use shuttle_runtime::{Args, AxumWasm, Legacy, StorageManagerType};
use tonic::transport::Server;
use tracing::trace;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    let args = Args::parse();

    let fmt_layer = fmt::layer();
    let filter_layer = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new("info"))
        .unwrap();

    tracing_subscriber::registry()
        .with(filter_layer)
        .with(fmt_layer)
        .init();

    trace!(args = ?args, "parsed args");

    let addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), args.port);

    let provisioner_address = args.provisioner_address;
    let mut server_builder =
        Server::builder().http2_keepalive_interval(Some(Duration::from_secs(60)));

    let router = if args.legacy {
        match args.storage_manager_type {
            StorageManagerType::Artifacts => {
                let legacy = Legacy::new(
                    provisioner_address,
                    ArtifactsStorageManager::new(args.storage_manager_path),
                );

                let svc = RuntimeServer::new(legacy);
                server_builder.add_service(svc)
            }
            StorageManagerType::WorkingDir => {
                let legacy = Legacy::new(
                    provisioner_address,
                    WorkingDirStorageManager::new(args.storage_manager_path),
                );

                let svc = RuntimeServer::new(legacy);
                server_builder.add_service(svc)
            }
        }
    } else if args.axum {
        let axum = AxumWasm::default();
        let svc = RuntimeServer::new(axum);
        server_builder.add_service(svc)
    } else {
        panic!("No runtime was selected");
    };

    router.serve(addr).await.unwrap();
}
