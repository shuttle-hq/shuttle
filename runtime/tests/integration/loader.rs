use shuttle_proto::runtime::{LoadRequest, StartRequest, StopReason, SubscribeStopRequest};

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
        runtime: _runtime, // Keep it to not be dropped and have the process killed.
    } = spawn_runtime(project_path, "bind-panic").await.unwrap();

    let load_request = tonic::Request::new(LoadRequest {
        path: bin_path,
        service_name,
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
    };

    runtime_client
        .start(tonic::Request::new(start_request))
        .await
        .unwrap();

    let reason = stream.message().await.unwrap().unwrap();

    assert_eq!(reason.reason, StopReason::Crash as i32);
    assert_ne!(reason.message, "<no panic message>");
    assert_eq!(reason.message, "panic in bind");
}

#[tokio::test]
async fn bind_panic_owned() {
    let project_path = format!(
        "{}/tests/resources/bind-panic-owned",
        env!("CARGO_MANIFEST_DIR")
    );

    let TestRuntime {
        bin_path,
        service_name,
        secrets,
        mut runtime_client,
        runtime_address,
        runtime: _runtime, // Keep it to not be dropped and have the process killed.
    } = spawn_runtime(project_path, "bind-panic-owned")
        .await
        .unwrap();

    let load_request = tonic::Request::new(LoadRequest {
        path: bin_path,
        service_name,
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
    };

    runtime_client
        .start(tonic::Request::new(start_request))
        .await
        .unwrap();

    let reason = stream.message().await.unwrap().unwrap();

    assert_eq!(reason.reason, StopReason::Crash as i32);
    assert_ne!(reason.message, "<no panic message>");
    assert_eq!(reason.message, "panic in bind");
}
#[tokio::test]
async fn loader_panic() {
    let project_path = format!(
        "{}/tests/resources/loader-panic",
        env!("CARGO_MANIFEST_DIR")
    );

    let TestRuntime {
        bin_path,
        service_name,
        secrets,
        mut runtime_client,
        runtime_address: _,
        runtime: _runtime, // Keep it to not be dropped and have the process killed.
    } = spawn_runtime(project_path, "loader-panic").await.unwrap();

    let load_request = tonic::Request::new(LoadRequest {
        path: bin_path,
        service_name,
        resources: Default::default(),
        secrets,
    });

    let load_response = runtime_client.load(load_request).await.unwrap();
    let message = load_response.into_inner().message;
    assert_eq!(message, "panic in loader");
    assert_ne!(message, "<no panic message>");
}

#[tokio::test]
async fn loader_panic_owned() {
    let project_path = format!(
        "{}/tests/resources/loader-panic-owned",
        env!("CARGO_MANIFEST_DIR")
    );

    let TestRuntime {
        bin_path,
        service_name,
        secrets,
        mut runtime_client,
        runtime_address: _,
        runtime: _runtime, // Keep it to not be dropped and have the process killed.
    } = spawn_runtime(project_path, "loader-panic-owned")
        .await
        .unwrap();

    let load_request = tonic::Request::new(LoadRequest {
        path: bin_path,
        service_name,
        resources: Default::default(),
        secrets,
    });

    let load_response = runtime_client.load(load_request).await.unwrap();
    let message = load_response.into_inner().message;
    assert_ne!(message, "<no panic message>");
    assert_eq!(message, "panic in loader");
}
