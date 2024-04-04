use std::{
    net::{Ipv4Addr, SocketAddr},
    time::Duration,
};

use portpicker::pick_unused_port;
use shuttle_proto::provisioner::{
    self,
    provisioner_server::{Provisioner, ProvisionerServer},
};
use tonic::transport::Server;

pub async fn get_mocked_provisioner_client(provisioner: impl Provisioner) -> provisioner::Client {
    let provisioner_addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), pick_unused_port().unwrap());
    let provisioner_uri = format!("http://{}", provisioner_addr);
    tokio::spawn(async move {
        Server::builder()
            .add_service(ProvisionerServer::new(provisioner))
            .serve(provisioner_addr)
            .await
    });

    // Wait for the provisioner server to start before creating a client.
    tokio::time::sleep(Duration::from_millis(200)).await;

    provisioner::get_client(provisioner_uri.parse().unwrap()).await
}
