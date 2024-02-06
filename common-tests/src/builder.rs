use std::{
    net::{Ipv4Addr, SocketAddr},
    time::Duration,
};

use portpicker::pick_unused_port;
use shuttle_proto::builder::{
    self,
    builder_server::{Builder, BuilderServer},
};
use tonic::transport::Server;

pub async fn get_mocked_builder_client(builder: impl Builder) -> builder::Client {
    let builder_addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), pick_unused_port().unwrap());
    let builder_uri = format!("http://{}", builder_addr);
    tokio::spawn(async move {
        Server::builder()
            .add_service(BuilderServer::new(builder))
            .serve(builder_addr)
            .await
    });

    // Wait for the builder server to start before creating a client.
    tokio::time::sleep(Duration::from_millis(200)).await;

    builder::get_client(builder_uri.parse().unwrap()).await
}
