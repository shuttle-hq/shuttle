use std::{
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};

use anyhow::Context;
use shuttle_common::{
    claims::{ClaimLayer, ClaimService, InjectPropagation, InjectPropagationLayer},
    deployment::Environment,
};
use shuttle_proto::runtime::runtime_client;
use tokio::process;
use tonic::transport::{Channel, Endpoint};
use tower::ServiceBuilder;
use tracing::{info, trace};

pub async fn start(
    wasm: bool,
    environment: Environment,
    provisioner_address: &str,
    auth_uri: Option<&String>,
    port: u16,
    runtime_executable: PathBuf,
    project_path: &Path,
) -> anyhow::Result<(
    process::Child,
    runtime_client::RuntimeClient<ClaimService<InjectPropagation<Channel>>>,
)> {
    let port = &port.to_string();
    let environment = &environment.to_string();

    let args = if wasm {
        vec!["--port", port]
    } else {
        let mut args = vec![
            "--port",
            port,
            "--provisioner-address",
            provisioner_address,
            "--env",
            environment,
        ];

        if let Some(auth_uri) = auth_uri {
            args.append(&mut vec!["--auth-uri", auth_uri]);
        }

        args
    };

    info!(
        "Spawning runtime process: {} {}",
        runtime_executable.display(),
        args.join(" ")
    );
    let runtime = process::Command::new(
        dunce::canonicalize(runtime_executable).context("canonicalize path of executable")?,
    )
    .current_dir(project_path)
    .args(&args)
    .stdout(Stdio::piped())
    .kill_on_drop(true)
    .spawn()
    .context("spawning runtime process")?;

    info!("connecting runtime client");
    let conn = Endpoint::new(format!("http://127.0.0.1:{port}"))
        .context("creating runtime client endpoint")?
        .connect_timeout(Duration::from_secs(5));

    // Wait for the spawned process to open the control port.
    // Connecting instantly does not give it enough time.
    let channel = tokio::time::timeout(Duration::from_millis(7000), async move {
        let mut ms = 5;
        loop {
            if let Ok(channel) = conn.connect().await {
                break channel;
            }
            trace!("waiting for runtime control port to open");
            // exponential backoff
            tokio::time::sleep(Duration::from_millis(ms)).await;
            ms *= 2;
        }
    })
    .await
    .context("runtime control port did not open in time")?;

    let channel = ServiceBuilder::new()
        .layer(ClaimLayer)
        .layer(InjectPropagationLayer)
        .service(channel);
    let runtime_client = runtime_client::RuntimeClient::new(channel);

    Ok((runtime, runtime_client))
}
