use shuttle_proto::runtime::{LoadRequest, StartRequest, StopReason, SubscribeStopRequest};
use shuttle_service::Environment;

use crate::helpers::{spawn_runtime, TestRuntime};

#[tokio::test]
async fn bind_panic() {
    let project_path = format!("{}/tests/resources/bind-panic", env!("CARGO_MANIFEST_DIR"));

    let TestRuntime {
        bin_path,
        secrets,
        mut runtime_client,
        runtime_address,
        runtime: _runtime, // Keep it to not be dropped and have the process killed.
    } = spawn_runtime(project_path.as_str()).await.unwrap();

    let load_request = tonic::Request::new(LoadRequest {
        path: bin_path,
        env: Environment::Local.to_string(),
        project_name: "bind-panic".to_owned(),
        resources: Default::default(),
        secrets,
    });
    runtime_client.load(load_request).await.unwrap();
    let mut stream = runtime_client
        .subscribe_stop(tonic::Request::new(SubscribeStopRequest {}))
        .await
        .unwrap()
        .into_inner();

    let start_request = StartRequest {
        ip: runtime_address.to_string(),
        resources: Default::default(),
    };
    runtime_client
        .start(tonic::Request::new(start_request))
        .await
        .unwrap();

    let resp = stream.message().await.unwrap().unwrap();
    assert_eq!(resp.reason, StopReason::Crash as i32);
    assert_eq!(resp.message, "panic in bind");
}

#[tokio::test]
async fn loader_panic() {
    let project_path = format!(
        "{}/tests/resources/loader-panic",
        env!("CARGO_MANIFEST_DIR")
    );

    let TestRuntime {
        bin_path,
        secrets,
        mut runtime_client,
        runtime_address: _,
        runtime: _runtime, // Keep it to not be dropped and have the process killed.
    } = spawn_runtime(project_path.as_str()).await.unwrap();

    let load_request = tonic::Request::new(LoadRequest {
        path: bin_path,
        env: Environment::Local.to_string(),
        project_name: "loader-panic".to_owned(),
        resources: Default::default(),
        secrets,
    });
    let resp = runtime_client
        .load(load_request)
        .await
        .unwrap()
        .into_inner();

    assert_eq!(resp.message, "panic in load");
}

#[tokio::test]
async fn main_panic() {
    let project_path = format!("{}/tests/resources/main-panic", env!("CARGO_MANIFEST_DIR"));

    let TestRuntime {
        bin_path,
        secrets,
        mut runtime_client,
        runtime_address,
        runtime: _runtime, // Keep it to not be dropped and have the process killed.
    } = spawn_runtime(project_path.as_str()).await.unwrap();

    let load_request = tonic::Request::new(LoadRequest {
        path: bin_path,
        env: Environment::Local.to_string(),
        project_name: "main-panic".to_owned(),
        resources: Default::default(),
        secrets,
    });

    runtime_client.load(load_request).await.unwrap();
    let mut stream = runtime_client
        .subscribe_stop(tonic::Request::new(SubscribeStopRequest {}))
        .await
        .unwrap()
        .into_inner();

    let start_request = StartRequest {
        ip: runtime_address.to_string(),
        resources: Default::default(),
    };
    runtime_client
        .start(tonic::Request::new(start_request))
        .await
        .unwrap();

    let resp = stream.message().await.unwrap().unwrap();
    assert_eq!(resp.reason, StopReason::Crash as i32);
    assert_eq!(resp.message, "panic in main");
}
