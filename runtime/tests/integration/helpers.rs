use std::{
    collections::HashMap,
    net::{Ipv4Addr, SocketAddr},
    path::{Path, PathBuf},
};

use anyhow::Result;
use async_trait::async_trait;
use shuttle_common::claims::{ClaimService, InjectPropagation};
use shuttle_proto::{
    provisioner::{
        provisioner_server::{Provisioner, ProvisionerServer},
        DatabaseDeletionResponse, DatabaseRequest, DatabaseResponse,
    },
    runtime::{self, runtime_client::RuntimeClient},
};
use shuttle_service::builder::{build_workspace, BuiltService};
use tokio::process::Child;
use tonic::{
    transport::{Channel, Server},
    Request, Response, Status,
};

pub struct TestRuntime {
    pub runtime_client: RuntimeClient<ClaimService<InjectPropagation<Channel>>>,
    pub bin_path: String,
    pub service_name: String,
    pub runtime_address: SocketAddr,
    pub secrets: HashMap<String, String>,
    pub runtime: Child,
}

pub async fn spawn_runtime(project_path: String, service_name: &str) -> Result<TestRuntime> {
    let provisioner_address = SocketAddr::new(
        Ipv4Addr::LOCALHOST.into(),
        portpicker::pick_unused_port().unwrap(),
    );
    let runtime_port = portpicker::pick_unused_port().unwrap();
    let runtime_address = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), runtime_port);

    let (tx, _) = crossbeam_channel::unbounded();
    let runtimes = build_workspace(Path::new(&project_path), false, tx, false).await?;

    let secrets: HashMap<String, String> = Default::default();

    let BuiltService {
        executable_path,
        is_wasm,
        ..
    } = runtimes[0].clone();

    start_provisioner(DummyProvisioner, provisioner_address);

    // TODO: update this to work with shuttle-next projects, see cargo-shuttle local run
    let runtime_path = || executable_path.clone();

    let (runtime, runtime_client) = runtime::start(
        is_wasm,
        runtime::StorageManagerType::WorkingDir(PathBuf::from(project_path.clone())),
        &format!("http://{}", provisioner_address),
        None,
        runtime_port,
        runtime_path,
    )
    .await?;

    Ok(TestRuntime {
        runtime_client,
        bin_path: executable_path
            .into_os_string()
            .into_string()
            .expect("to convert path to string"),
        service_name: service_name.to_string(),
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
}
