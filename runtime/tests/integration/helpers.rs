use std::{
    collections::HashMap,
    net::{Ipv4Addr, SocketAddr},
    path::{Path, PathBuf},
};

use anyhow::Result;
use async_trait::async_trait;
use bollard::Docker;
use shuttle_proto::{
    provisioner::{
        provisioner_server::{Provisioner, ProvisionerServer},
        DatabaseRequest, DatabaseResponse,
    },
    runtime::{self, runtime_client::RuntimeClient},
};
use shuttle_service::builder::{build_crate, Runtime};
use shuttle_service::database::Type;
use tokio::task::JoinHandle;
use tonic::{
    transport::{self, Channel, Server},
    Request, Response, Status,
};

pub struct TestRuntime {
    pub runtime_client: RuntimeClient<Channel>,
    pub bin_path: String,
    pub service_name: String,
    pub runtime_address: SocketAddr,
    pub secrets: HashMap<String, String>,
}

pub async fn spawn_runtime(project_path: String, service_name: &str) -> Result<TestRuntime> {
    let provisioner_address = SocketAddr::new(
        Ipv4Addr::LOCALHOST.into(),
        portpicker::pick_unused_port().unwrap(),
    );
    let runtime_port = portpicker::pick_unused_port().unwrap();
    let runtime_address = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), runtime_port);

    let (tx, _) = crossbeam_channel::unbounded();
    let runtime = build_crate(Path::new(&project_path), false, tx).await?;

    let secrets: HashMap<String, String> = Default::default();

    let (is_wasm, bin_path) = match runtime {
        Runtime::Next(path) => (true, path),
        Runtime::Legacy(path) => (false, path),
    };

    let provisioner = DummyProvisioner::new()?;
    let provisioner_server = provisioner.start(provisioner_address);

    // TODO: update this to work with shuttle-next projects, see cargo-shuttle local run
    let runtime_path = || bin_path.clone();

    let (_, runtime_client) = runtime::start(
        is_wasm,
        runtime::StorageManagerType::WorkingDir(PathBuf::from(project_path.clone())),
        &format!("http://{}", provisioner_address),
        runtime_port,
        runtime_path,
    )
    .await
    .map_err(|err| {
        provisioner_server.abort();

        err
    })?;

    Ok(TestRuntime {
        runtime_client,
        bin_path: bin_path
            .into_os_string()
            .into_string()
            .expect("to convert path to string"),
        service_name: service_name.to_string(),
        runtime_address,
        secrets,
    })
}

/// A dummy provisioner for tests, a provisioner connection is required
/// to start a project runtime.
pub struct DummyProvisioner {
    #[allow(dead_code)]
    docker: Docker,
}

impl DummyProvisioner {
    pub fn new() -> Result<Self> {
        Ok(Self {
            docker: Docker::connect_with_local_defaults()?,
        })
    }

    pub fn start(self, address: SocketAddr) -> JoinHandle<Result<(), transport::Error>> {
        tokio::spawn(async move {
            Server::builder()
                .add_service(ProvisionerServer::new(self))
                .serve(address)
                .await
        })
    }

    #[allow(dead_code)]
    async fn get_db_connection_string(
        &self,
        _service_name: &str,
        _db_type: Type,
    ) -> Result<DatabaseResponse, Status> {
        panic!("did not expect any runtime test to use dbs")
    }
}

#[async_trait]
impl Provisioner for DummyProvisioner {
    async fn provision_database(
        &self,
        _request: Request<DatabaseRequest>,
    ) -> Result<Response<DatabaseResponse>, Status> {
        panic!("did not expect any runtime test to use dbs")
    }
}
