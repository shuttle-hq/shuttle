mod args;
mod legacy;
mod next;
pub mod provisioner_factory;

use std::net::{Ipv4Addr, SocketAddr};

pub use args::Args;
pub use legacy::Legacy;
pub use next::Next;
use shuttle_proto::runtime::runtime_server::RuntimeServer;
use tonic::transport::{Endpoint, Server};

pub async fn start_legacy() {
    // starting the router on 8002 to avoid conflicts
    let grpc_address = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 8002);
    let provisioner_address = Endpoint::from_static("http://127.0.0.1:8000");

    let legacy = Legacy::new(provisioner_address);
    let svc = RuntimeServer::new(legacy);

    let router = Server::builder().add_service(svc);

    router.serve(grpc_address).await.unwrap();
}
