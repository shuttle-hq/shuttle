use std::net::{Ipv4Addr, SocketAddr};

use async_trait::async_trait;
use portpicker::pick_unused_port;
use shuttle_proto::provisioner::{
    provisioner_server::{Provisioner, ProvisionerServer},
    DatabaseDeletionResponse, DatabaseRequest, DatabaseResponse, Ping, Pong,
};
use tonic::transport::Server;

struct ProvisionerMock;

#[async_trait]
impl Provisioner for ProvisionerMock {
    async fn provision_database(
        &self,
        _request: tonic::Request<DatabaseRequest>,
    ) -> Result<tonic::Response<DatabaseResponse>, tonic::Status> {
        panic!("no run tests should request a db");
    }

    async fn delete_database(
        &self,
        _request: tonic::Request<DatabaseRequest>,
    ) -> Result<tonic::Response<DatabaseDeletionResponse>, tonic::Status> {
        panic!("no run tests should delete a db");
    }

    async fn health_check(
        &self,
        _request: tonic::Request<Ping>,
    ) -> Result<tonic::Response<Pong>, tonic::Status> {
        panic!("no run tests should do a health check");
    }
}

/// Start a mocked provisioner and return the port it started on
pub async fn get_mocked_provisioner() -> u16 {
    let provisioner = ProvisionerMock;

    let port = pick_unused_port().unwrap();
    let provisioner_addr = SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), port);
    tokio::spawn(async move {
        Server::builder()
            .add_service(ProvisionerServer::new(provisioner))
            .serve(provisioner_addr)
            .await
    });

    port
}
