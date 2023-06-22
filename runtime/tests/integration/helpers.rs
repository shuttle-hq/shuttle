use std::{
    collections::HashMap,
    net::{Ipv4Addr, SocketAddr},
    path::{Path, PathBuf},
};

use anyhow::Result;
use async_trait::async_trait;
use shuttle_common::claims::{ClaimService, InjectPropagation};
use shuttle_proto::{
    auth::{
        auth_server::{Auth, AuthServer},
        ApiKeyRequest, ConvertCookieRequest, LogoutRequest, NewUser, PublicKeyRequest,
        PublicKeyResponse, ResetKeyRequest, ResultResponse, TokenResponse, UserRequest,
        UserResponse,
    },
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
use ulid::Ulid;

pub struct TestRuntime {
    pub runtime_client: RuntimeClient<ClaimService<InjectPropagation<Channel>>>,
    pub bin_path: String,
    pub service_name: String,
    pub deployment_id: Ulid,
    pub runtime_address: SocketAddr,
    pub secrets: HashMap<String, String>,
    pub runtime: Child,
}

pub async fn spawn_runtime(project_path: String, service_name: &str) -> Result<TestRuntime> {
    let provisioner_address = SocketAddr::new(
        Ipv4Addr::LOCALHOST.into(),
        portpicker::pick_unused_port().unwrap(),
    );
    let auth_address = SocketAddr::new(
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
    start_auth(DummyAuth, auth_address);

    // TODO: update this to work with shuttle-next projects, see cargo-shuttle local run
    let runtime_path = || executable_path.clone();

    let (runtime, runtime_client) = runtime::start(
        is_wasm,
        runtime::StorageManagerType::WorkingDir(PathBuf::from(project_path.clone())),
        &format!("http://{}", provisioner_address),
        Some(&format!("http://{}", auth_address)),
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
        deployment_id: Ulid::new(),
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

/// A dummy auth server for tests, an auth connection is required
/// to start a project runtime.
pub struct DummyAuth;

fn start_auth(auth: DummyAuth, address: SocketAddr) {
    tokio::spawn(async move {
        Server::builder()
            .add_service(AuthServer::new(auth))
            .serve(address)
            .await
    });
}

#[async_trait]
impl Auth for DummyAuth {
    async fn get_user_request(
        &self,
        _request: Request<UserRequest>,
    ) -> Result<Response<UserResponse>, Status> {
        panic!("did not expect any runtime to get user")
    }

    async fn post_user_request(
        &self,
        _request: Request<NewUser>,
    ) -> Result<Response<UserResponse>, Status> {
        panic!("did not expect any runtime test to create users")
    }

    async fn login(
        &self,
        _request: Request<UserRequest>,
    ) -> Result<Response<UserResponse>, Status> {
        panic!("did not expect any runtime test to login user")
    }

    async fn logout(
        &self,
        mut _request: Request<LogoutRequest>,
    ) -> Result<Response<ResultResponse>, Status> {
        panic!("did not expect any runtime test to logout user")
    }

    async fn convert_api_key(
        &self,
        _request: Request<ApiKeyRequest>,
    ) -> Result<Response<TokenResponse>, Status> {
        panic!("did not expect any runtime test to convert api key")
    }

    async fn convert_cookie(
        &self,
        _request: Request<ConvertCookieRequest>,
    ) -> Result<Response<TokenResponse>, Status> {
        panic!("did not expect any runtime test to convert cookie")
    }

    async fn reset_api_key(
        &self,
        _request: Request<ResetKeyRequest>,
    ) -> Result<Response<ResultResponse>, Status> {
        panic!("did not expect any runtime test to reset api key")
    }

    async fn public_key(
        &self,
        _request: Request<PublicKeyRequest>,
    ) -> Result<Response<PublicKeyResponse>, Status> {
        panic!("did not expect any runtime test to request public key")
    }
}
