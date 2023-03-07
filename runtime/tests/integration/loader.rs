use std::time::Duration;

use shuttle_proto::runtime::{LoadRequest, StartRequest};
use uuid::Uuid;

use crate::helpers::{spawn_runtime, TestRuntime};

/// This test does panic, but the panic happens in a spawned task inside the project runtime,
/// so we get this output: `thread 'tokio-runtime-worker' panicked at 'panic in bind', src/main.rs:6:9`,
/// but `should_panic(expected = "panic in bind")` doesn't catch it.
#[tokio::test]
#[should_panic(expected = "panic in bind")]
async fn bind_panic() {
    let project_path = "/home/oddgrd/dev/shuttle/runtime/tests/resources/bind-panic".to_owned();

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

    let start_request = StartRequest {
        deployment_id: Uuid::default().as_bytes().to_vec(),
        ip: runtime_address.to_string(),
    };

    // I also tried this without spawning, but it gave the same result. Panic but it isn't caught.
    tokio::spawn(async move {
        runtime_client
            .start(tonic::Request::new(start_request))
            .await
            .unwrap();
        // Give it a second to panic.
        tokio::time::sleep(Duration::from_secs(1)).await;
    })
    .await
    .unwrap();
}
