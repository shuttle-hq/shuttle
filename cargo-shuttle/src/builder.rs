use std::fs;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use anyhow::{bail, Context, Result};
use cargo_metadata::{Metadata, Package, Target};
use shuttle_common::constants::RUNTIME_NAME;
use shuttle_macros::find_user_main_fn;
use tokio::io::AsyncBufReadExt;
use tracing::{error, trace};

/// This represents a compiled Shuttle service
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BuiltService {
    pub workspace_path: PathBuf,
    pub target_name: String,
    pub executable_path: PathBuf,
}

/// Builds Shuttle service in given project directory
pub async fn build_workspace(
    project_path: &Path,
    release_mode: bool,
    tx: tokio::sync::mpsc::Sender<String>,
) -> Result<BuiltService> {
    let project_path = project_path.to_owned();
    let manifest_path = project_path.join("Cargo.toml");
    if !manifest_path.exists() {
        bail!("Cargo manifest file not found: {}", manifest_path.display());
    }

    // Cargo's "Downloading ..." lines are quite verbose.
    // Instead, a custom message is printed if the download takes significant time.
    // Cargo seems to have similar logic, where it prints nothing if this step takes little time.
    let mut command = tokio::process::Command::new("cargo");
    command
        .arg("fetch")
        .arg("--manifest-path")
        .arg(&manifest_path)
        .arg("--color=always")
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let notification = tokio::spawn({
        let tx = tx.clone();
        async move {
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
            tx.send("      Downloading crates...".into())
                .await
                .expect("log receiver to exist");
        }
    });
    if !command.status().await?.success() {
        tx.send("      Failed to fetch crates".into())
            .await
            .expect("log receiver to exist");
    }
    notification.abort();

    let metadata = async_cargo_metadata(manifest_path.as_path()).await?;
    let (package, target) = find_first_shuttle_package(&metadata)?;

    let service = cargo_build(
        package,
        target,
        release_mode,
        project_path.clone(),
        metadata.target_directory.clone(),
        tx.clone(),
    )
    .await?;
    trace!("package compiled");

    Ok(service)
}

pub async fn async_cargo_metadata(manifest_path: &Path) -> Result<Metadata> {
    let metadata = {
        // Modified implementaion of `cargo_metadata::MetadataCommand::exec` (from v0.15.3).
        // Uses tokio Command instead of std, to make this operation non-blocking.
        let mut cmd = tokio::process::Command::from(
            cargo_metadata::MetadataCommand::new()
                .manifest_path(manifest_path)
                .cargo_command(),
        );

        let output = cmd.output().await?;
        if !output.status.success() {
            return Err(cargo_metadata::Error::CargoMetadata {
                stderr: String::from_utf8(output.stderr)?,
            })?;
        }
        let json = std::str::from_utf8(&output.stdout)?
            .lines()
            .find(|line| line.starts_with('{'))
            .ok_or(cargo_metadata::Error::NoJson)?;
        cargo_metadata::MetadataCommand::parse(json)?
    };
    trace!("Cargo metadata parsed");

    Ok(metadata)
}

/// Find crates with a runtime dependency and main macro
fn find_shuttle_packages(metadata: &Metadata) -> Result<Vec<(Package, Target)>> {
    let mut packages = Vec::new();
    trace!("Finding Shuttle-related packages");
    for member in metadata.workspace_packages() {
        let has_runtime_dep = member
            .dependencies
            .iter()
            .any(|dependency| dependency.name == RUNTIME_NAME);
        if !has_runtime_dep {
            trace!("Skipping {}, no shuttle-runtime dependency", member.name);
            continue;
        }

        let mut target = None;
        for t in member.targets.iter() {
            if t.is_bin()
                && find_user_main_fn(
                    &fs::read_to_string(t.src_path.as_std_path())
                        .context("reading to check for shuttle macro")?,
                )
                .context("parsing rust file when checking for shuttle macro")?
                .is_some()
            {
                target = Some(t);
                break;
            }
        }
        let Some(target) = target else {
            trace!(
                "Skipping {}, no binary target with a #[shuttle_runtime::main] macro",
                member.name
            );
            continue;
        };

        trace!("Found {}", member.name);
        packages.push((member.to_owned(), target.to_owned()));
    }

    Ok(packages)
}

pub fn find_first_shuttle_package(metadata: &Metadata) -> Result<(Package, Target)> {
    find_shuttle_packages(metadata)?.into_iter().next().context(
        "Expected at least one target that Shuttle can build. \
        Make sure your crate has a binary target that uses a fully qualified `#[shuttle_runtime::main]`.",
    )
}

async fn cargo_build(
    package: Package,
    target: Target,
    release_mode: bool,
    project_path: PathBuf,
    target_path: impl Into<PathBuf>,
    tx: tokio::sync::mpsc::Sender<String>,
) -> Result<BuiltService> {
    let manifest_path = project_path.join("Cargo.toml");
    if !manifest_path.exists() {
        bail!("Cargo manifest file not found: {}", manifest_path.display());
    }
    let target_path = target_path.into();

    // TODO?: Use https://crates.io/crates/escargot instead

    let mut cmd = tokio::process::Command::new("cargo");
    cmd.arg("build")
        .arg("--manifest-path")
        .arg(manifest_path)
        .arg("--color=always") // piping disables auto color, but we want it
        .current_dir(project_path.as_path());

    if package.features.contains_key("shuttle") {
        cmd.arg("--no-default-features").arg("--features=shuttle");
    }
    cmd.arg("--package").arg(package.name.as_str());
    cmd.arg("--bin").arg(target.name.as_str());

    let profile = if release_mode {
        cmd.arg("--release");
        "release"
    } else {
        "debug"
    };

    cmd.stderr(Stdio::piped());
    cmd.stdout(Stdio::null());
    let mut handle = cmd.spawn()?;
    let reader = tokio::io::BufReader::new(handle.stderr.take().unwrap());
    tokio::spawn(async move {
        let mut lines = reader.lines();
        while let Some(line) = lines.next_line().await.unwrap() {
            let _ = tx
                .send(line)
                .await
                .map_err(|error| error!(error = &error as &dyn std::error::Error));
        }
    });
    let status = handle.wait().await?;
    if !status.success() {
        bail!("Build failed.");
    }

    let mut path: PathBuf = [
        project_path.clone(),
        target_path.clone(),
        profile.into(),
        target.name.as_str().into(),
    ]
    .iter()
    .collect();
    path.set_extension(std::env::consts::EXE_EXTENSION);

    Ok(BuiltService {
        workspace_path: project_path.clone(),
        target_name: target.name,
        executable_path: path,
    })
}
