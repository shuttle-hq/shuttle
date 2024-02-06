use std::{
    path::{Path, PathBuf},
    process::Stdio,
};

use anyhow::Context;
use shuttle_common::deployment::Environment;
use shuttle_proto::runtime;
use tokio::process;
use tracing::info;

pub async fn start(
    wasm: bool,
    environment: Environment,
    provisioner_address: &str,
    auth_uri: Option<&String>,
    port: u16,
    runtime_executable: PathBuf,
    project_path: &Path,
) -> anyhow::Result<(process::Child, runtime::Client)> {
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
        args = %format!("{} {}", runtime_executable.display(), args.join(" ")),
        "Spawning runtime process",
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

    let runtime_client = runtime::get_client(port).await?;

    Ok((runtime, runtime_client))
}
