use shuttle_proto::runtime::{LoadRequest, StartRequest, StopReason, SubscribeStopRequest};
use uuid::Uuid;

use crate::helpers::{spawn_runtime, TestRuntime};

#[tokio::test]
async fn bind_panic() {
    let project_path = format!("{}/tests/resources/bind-panic", env!("CARGO_MANIFEST_DIR"));

    let TestRuntime {
        bin_path,
        service_name,
        secrets,
        mut runtime_client,
        runtime_address,
    } = spawn_runtime(project_path, "bind-panic").await.unwrap();

    let load_request = tonic::Request::new(LoadRequest {
        path: bin_path,
        service_name,
        secrets,
    });

    let _ = runtime_client.load(load_request).await.unwrap();

    let mut stream = runtime_client
        .subscribe_stop(tonic::Request::new(SubscribeStopRequest {}))
        .await
        .unwrap()
        .into_inner();

    let start_request = StartRequest {
        deployment_id: Uuid::default().as_bytes().to_vec(),
        ip: runtime_address.to_string(),
    };

    runtime_client
        .start(tonic::Request::new(start_request))
        .await
        .unwrap();

    let reason = stream.message().await.unwrap().unwrap();

    assert_eq!(reason.reason, StopReason::Crash as i32);
    assert_eq!(reason.message, "panic in bind");
}
