use std::{
    net::{Ipv4Addr, SocketAddr},
    path::PathBuf,
    sync::Arc,
    time::Duration,
};

use async_trait::async_trait;
use portpicker::pick_unused_port;
use shuttle_common::{claims::Claim, constants::EXECUTABLE_DIRNAME};
use shuttle_common_tests::logger::{get_mocked_logger_client, MockedLogger};
use shuttle_proto::{
    logger::Batcher,
    provisioner::{
        provisioner_server::{Provisioner, ProvisionerServer},
        ContainerRequest, ContainerResponse, DatabaseDeletionResponse, DatabaseRequest,
        DatabaseResponse, Ping, Pong,
    },
    resource_recorder::{ResourceResponse, ResourcesResponse, ResultResponse},
    runtime::{StopReason, SubscribeStopResponse},
};
use tokio::{
    process::Command,
    sync::{oneshot, Mutex},
    time::sleep,
};
use tonic::transport::Server;
use ulid::Ulid;
use uuid::Uuid;

use shuttle_deployer::{
    deployment::Built, error, persistence::resource::ResourceManager, RuntimeManager,
};

const RESOURCES_PATH: &str = "tests/resources";

async fn kill_old_deployments() -> error::Result<()> {
    Ok(())
}

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

    async fn provision_arbitrary_container(
        &self,
        _req: tonic::Request<ContainerRequest>,
    ) -> Result<tonic::Response<ContainerResponse>, tonic::Status> {
        panic!("no run tests should request container")
    }

    async fn health_check(
        &self,
        _request: tonic::Request<Ping>,
    ) -> Result<tonic::Response<Pong>, tonic::Status> {
        panic!("no run tests should do a health check");
    }
}

async fn get_runtime_manager() -> Arc<Mutex<RuntimeManager>> {
    let provisioner_addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), pick_unused_port().unwrap());

    tokio::spawn(async move {
        Server::builder()
            .add_service(ProvisionerServer::new(ProvisionerMock))
            .serve(provisioner_addr)
            .await
            .unwrap();
    });

    let logger_client = Batcher::wrap(get_mocked_logger_client(MockedLogger).await);

    RuntimeManager::new(format!("http://{}", provisioner_addr), logger_client, None)
}

#[derive(Clone)]
struct StubResourceManager;

#[async_trait]
impl ResourceManager for StubResourceManager {
    type Err = std::io::Error;

    async fn insert_resources(
        &mut self,
        _resources: Vec<shuttle_proto::resource_recorder::record_request::Resource>,
        _service_id: &ulid::Ulid,
        _claim: Claim,
    ) -> Result<ResultResponse, Self::Err> {
        Ok(ResultResponse {
            success: true,
            message: "dummy impl".to_string(),
        })
    }
    async fn get_resources(
        &mut self,
        _service_id: &ulid::Ulid,
        _claim: Claim,
    ) -> Result<ResourcesResponse, Self::Err> {
        Ok(ResourcesResponse {
            success: true,
            message: "dummy impl".to_string(),
            resources: Vec::new(),
        })
    }

    async fn delete_resource(
        &mut self,
        _project_name: String,
        _service_id: &Ulid,
        _resource_type: shuttle_common::resource::Type,
        _claim: Claim,
    ) -> Result<ResultResponse, Self::Err> {
        Ok(ResultResponse {
            success: true,
            message: "dummy impl".to_string(),
        })
    }

    async fn get_resource(
        &mut self,
        _service_id: &ulid::Ulid,
        _resource_type: shuttle_common::resource::Type,
        _claim: Claim,
    ) -> Result<ResourceResponse, Self::Err> {
        Ok(ResourceResponse {
            success: true,
            message: "dummy impl".to_string(),
            resource: None,
        })
    }
}

// This test uses the kill signal to make sure a service does stop when asked to
#[tokio::test]
async fn can_be_killed() {
    let (built, path) = make_and_built("sleep-async").await;
    let id = built.id;
    let runtime_manager = get_runtime_manager().await;
    let (cleanup_send, cleanup_recv) = oneshot::channel();

    let handle_cleanup = |response: Option<SubscribeStopResponse>| {
        let response = response.unwrap();
        match (
            StopReason::try_from(response.reason).unwrap(),
            response.message,
        ) {
            (StopReason::Request, mes) if mes.is_empty() => cleanup_send.send(()).unwrap(),
            _ => panic!("expected stop due to request"),
        }
    };

    built
        .handle(
            StubResourceManager,
            runtime_manager.clone(),
            kill_old_deployments(),
            handle_cleanup,
            path.as_path(),
        )
        .await
        .unwrap();

    // Give it some time to start up
    sleep(Duration::from_secs(1)).await;

    // Send kill signal
    assert!(runtime_manager.lock().await.kill(&id).await);

    tokio::select! {
        _ = sleep(Duration::from_secs(1)) => panic!("cleanup should have been called"),
        Ok(()) = cleanup_recv => {}
    }
}

// This test does not use a kill signal to stop the service. Rather the service decided to stop on its own without errors
#[tokio::test]
async fn self_stop() {
    let (built, path) = make_and_built("sleep-async").await;
    let runtime_manager = get_runtime_manager().await;
    let (cleanup_send, cleanup_recv) = oneshot::channel();

    let handle_cleanup = |response: Option<SubscribeStopResponse>| {
        let response = response.unwrap();
        match (
            StopReason::try_from(response.reason).unwrap(),
            response.message,
        ) {
            (StopReason::End, mes) if mes.is_empty() => cleanup_send.send(()).unwrap(),
            _ => panic!("expected stop due to self end"),
        }
    };

    built
        .handle(
            StubResourceManager,
            runtime_manager.clone(),
            kill_old_deployments(),
            handle_cleanup,
            path.as_path(),
        )
        .await
        .unwrap();

    tokio::select! {
        _ = sleep(Duration::from_secs(5)) => panic!("cleanup should have been called as service stopped on its own"),
        Ok(()) = cleanup_recv => {},
    }

    // Prevent the runtime manager from dropping earlier, which will kill the processes it manages
    drop(runtime_manager);
}

// Test for panics in Service::bind
#[tokio::test]
async fn panic_in_bind() {
    let (built, path) = make_and_built("bind-panic").await;
    let runtime_manager = get_runtime_manager().await;
    let (cleanup_send, cleanup_recv) = oneshot::channel();

    let handle_cleanup = |response: Option<SubscribeStopResponse>| {
        let response = response.unwrap();
        match (
            StopReason::try_from(response.reason).unwrap(),
            response.message,
        ) {
            (StopReason::Crash, mes) if mes.contains("panic in bind") => {
                cleanup_send.send(()).unwrap()
            }
            (_, mes) => panic!("expected stop due to crash: {mes}"),
        }
    };

    built
        .handle(
            StubResourceManager,
            runtime_manager.clone(),
            kill_old_deployments(),
            handle_cleanup,
            path.as_path(),
        )
        .await
        .unwrap();

    tokio::select! {
        _ = sleep(Duration::from_secs(5)) => panic!("cleanup should have been called as service handle stopped after panic"),
        Ok(()) = cleanup_recv => {}
    }

    // Prevent the runtime manager from dropping earlier, which will kill the processes it manages
    drop(runtime_manager);
}

// Test for panics in the main function
#[tokio::test]
#[should_panic(expected = "Load(\"main panic\")")]
async fn panic_in_main() {
    let (built, path) = make_and_built("main-panic").await;
    let runtime_manager = get_runtime_manager().await;

    let handle_cleanup = |_result| panic!("service should never be started");

    let x = built
        .handle(
            StubResourceManager,
            runtime_manager.clone(),
            kill_old_deployments(),
            handle_cleanup,
            path.as_path(),
        )
        .await;
    println!("{:?}", x);

    x.unwrap();
}

async fn make_and_built(crate_name: &str) -> (Built, PathBuf) {
    let crate_dir: PathBuf = [RESOURCES_PATH, crate_name].iter().collect();

    Command::new("cargo")
        .args(["build"])
        .current_dir(&crate_dir)
        .spawn()
        .unwrap()
        .wait()
        .await
        .unwrap();

    let bin_name = if cfg!(target_os = "windows") {
        format!("{}.exe", crate_name)
    } else {
        crate_name.to_string()
    };

    let id = Uuid::new_v4();
    let exe_path = crate_dir.join("target/debug").join(bin_name);
    let new_dir = crate_dir.join(EXECUTABLE_DIRNAME);
    let new_exe_path = new_dir.join(id.to_string());

    std::fs::create_dir_all(new_dir).unwrap();
    std::fs::copy(exe_path, new_exe_path).unwrap();
    (
        Built {
            id,
            service_name: crate_name.to_string(),
            service_id: Ulid::new(),
            project_id: Ulid::new(),
            tracing_context: Default::default(),
            is_next: false,
            claim: Default::default(),
            secrets: Default::default(),
        },
        RESOURCES_PATH.into(), // is later joined with `service_name` to arrive at `crate_name`
    )
}
