use std::{
    collections::HashMap,
    net::{Ipv4Addr, SocketAddr},
    path::Path,
};

use anyhow::Result;
use async_trait::async_trait;
use shuttle_proto::{
    provisioner::{
        provisioner_server::{Provisioner, ProvisionerServer},
        DatabaseDeletionResponse, DatabaseDumpRequest, DatabaseDumpResponse, DatabaseRequest,
        DatabaseResponse, Ping, Pong,
    },
    runtime,
};
use shuttle_service::{builder::build_workspace, runner};
use tokio::process::Child;
use tonic::{transport::Server, Request, Response, Status};

pub struct TestRuntime {
    pub runtime_client: runtime::Client,
    pub bin_path: String,
    pub runtime_address: SocketAddr,
    pub secrets: HashMap<String, String>,
    pub runtime: Child,
}

pub async fn spawn_runtime(project_path: &str) -> Result<TestRuntime> {
    let runtime_port = portpicker::pick_unused_port().unwrap();
    let runtime_address = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), runtime_port);

    let (tx, _) = tokio::sync::mpsc::channel::<String>(256);
    let runtimes = build_workspace(Path::new(project_path), false, tx, false).await?;
    let service = runtimes[0].clone();

    let secrets: HashMap<String, String> = Default::default();

    start_provisioner(
        DummyProvisioner,
        SocketAddr::new(
            Ipv4Addr::LOCALHOST.into(),
            portpicker::pick_unused_port().unwrap(),
        ),
    );

    let runtime_executable = service.executable_path.clone();

    let (runtime, runtime_client) =
        runner::start(runtime_port, runtime_executable, Path::new(project_path)).await?;

    Ok(TestRuntime {
        runtime_client,
        bin_path: service
            .executable_path
            .into_os_string()
            .into_string()
            .expect("to convert path to string"),
        runtime_address,
        secrets,
        runtime,
    })
}

/// A dummy provisioner for tests, a provisioner connection is required
/// to start a project runtime.
pub struct DummyProvisioner;

fn start_provisioner(provisioner: DummyProvisioner, address: SocketAddr) {
    tokio::spawn(async move {
        Server::builder()
            .add_service(ProvisionerServer::new(provisioner))
            .serve(address)
            .await
    });
}

#[async_trait]
impl Provisioner for DummyProvisioner {
    async fn provision_database(
        &self,
        _request: Request<DatabaseRequest>,
    ) -> Result<Response<DatabaseResponse>, Status> {
        panic!("did not expect any runtime test to use dbs")
    }

    async fn delete_database(
        &self,
        _request: Request<DatabaseRequest>,
    ) -> Result<Response<DatabaseDeletionResponse>, Status> {
        panic!("did not expect any runtime test to delete dbs")
    }

    async fn dump_database(
        &self,
        _request: tonic::Request<DatabaseDumpRequest>,
    ) -> Result<tonic::Response<DatabaseDumpResponse>, tonic::Status> {
        panic!("did not expect any runtime test to dump a db");
    }

    async fn health_check(&self, _request: Request<Ping>) -> Result<Response<Pong>, Status> {
        panic!("did not expect any runtime test to do a provisioner health check")
    }
}
