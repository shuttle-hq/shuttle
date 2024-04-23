use std::{
    net::SocketAddr,
    path::{Path, PathBuf},
    process::Stdio,
};

use anyhow::Context;
use shuttle_proto::runtime;
use tokio::process;
use tracing::info;

pub async fn start(
    beta: bool,
    port: u16,
    // only used on beta. must match port.
    address: SocketAddr,
    runtime_executable: PathBuf,
    project_path: &Path,
) -> anyhow::Result<(process::Child, runtime::Client)> {
    let mut args = vec![];
    if beta {
        let addr_str = address.to_string();
        args.push("--beta".to_owned());
        args.push("--address".to_owned());
        args.push(addr_str);
    } else {
        let port_str = port.to_string();
        args.push("--port".to_owned());
        args.push(port_str);
    }

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

    // runtime might start on localhost or 0.0.0.0, but we can reach it on localhost:port
    let runtime_client = runtime::get_client(format!("http://localhost:{port}")).await?;

    Ok((runtime, runtime_client))
}
