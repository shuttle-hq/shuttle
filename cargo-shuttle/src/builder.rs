use std::fs;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use anyhow::{bail, Context, Result};
use cargo_metadata::{Metadata, Package, Target};
use shuttle_common::models::deployment::BuildArgsRust;
use shuttle_ifc::find_runtime_main_fn;
use tracing::trace;

/// This represents a compiled Shuttle service
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BuiltService {
    pub workspace_path: PathBuf,
    pub target_name: String,
    pub executable_path: PathBuf,
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
fn find_shuttle_packages(metadata: &Metadata) -> Result<Vec<(Package, Target, Option<String>)>> {
    let mut packages = Vec::new();
    trace!("Finding Shuttle-related packages");
    for member in metadata.workspace_packages() {
        let runtime_dep = member
            .dependencies
            .iter()
            .find(|dependency| dependency.name == "shuttle-runtime");
        let Some(runtime_dep) = runtime_dep else {
            trace!("Skipping {}, no shuttle-runtime dependency", member.name);
            continue;
        };
        let runtime_version = runtime_dep
            .req
            .comparators
            .first()
            // is "^0.X.0" when `shuttle-runtime = "0.X.0"` is in Cargo.toml, so strip the caret
            .and_then(|c| c.to_string().strip_prefix('^').map(ToOwned::to_owned));

        let mut target = None;
        for t in member.targets.iter() {
            if t.is_bin()
                && find_runtime_main_fn(
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
        packages.push((member.to_owned(), target.to_owned(), runtime_version));
    }

    Ok(packages)
}

/// Find first crate in workspace with a runtime dependency and main macro
pub fn find_first_shuttle_package(
    metadata: &Metadata,
) -> Result<(Package, Target, Option<String>)> {
    find_shuttle_packages(metadata)?.into_iter().next().context(
        "Expected at least one target that Shuttle can build. \
        Make sure your crate has a binary target that uses a fully qualified `#[shuttle_runtime::main]`.",
    )
}

pub async fn gather_rust_build_args(metadata: &Metadata) -> Result<BuildArgsRust> {
    let mut rust_build_args = BuildArgsRust::default();

    let (package, target, runtime_version) = find_first_shuttle_package(metadata)?;
    rust_build_args.package_name = Some(package.name.to_string());
    rust_build_args.binary_name = Some(target.name.clone());
    rust_build_args.shuttle_runtime_version = runtime_version;

    // activate shuttle feature if present
    let (no_default_features, features) = if package.features.contains_key("shuttle") {
        (true, Some(vec!["shuttle".to_owned()]))
    } else {
        (false, None)
    };
    rust_build_args.no_default_features = no_default_features;
    rust_build_args.features = features.map(|v| v.join(","));

    // TODO: have all of the above be configurable in CLI and Shuttle.toml

    Ok(rust_build_args)
}

pub async fn cargo_build(
    project_path: impl Into<PathBuf>,
    release_mode: bool,
    silent: bool,
) -> Result<BuiltService> {
    let project_path = project_path.into();
    let manifest_path = project_path.join("Cargo.toml");
    let metadata = async_cargo_metadata(manifest_path.as_path()).await?;
    let build_args = gather_rust_build_args(&metadata).await?;

    let package_name = build_args
        .package_name
        .as_ref()
        .context("missing package name argument")?;
    let binary_name = build_args
        .binary_name
        .as_ref()
        .context("missing binary name argument")?;
    let target_path = metadata.target_directory.into_std_path_buf();

    // TODO?: Use https://crates.io/crates/escargot instead

    let mut cmd = tokio::process::Command::new("cargo");
    cmd.arg("build")
        .arg("--manifest-path")
        .arg(manifest_path)
        .arg("--color=always") // piping disables auto color, but we want it
        .current_dir(project_path.as_path());

    if build_args.no_default_features {
        cmd.arg("--no-default-features");
    }
    if let Some(ref f) = build_args.features {
        cmd.arg("--features").arg(f);
    }
    cmd.arg("--package").arg(package_name);
    cmd.arg("--bin").arg(binary_name);

    let profile = if release_mode {
        cmd.arg("--release");
        "release"
    } else {
        "debug"
    };

    if silent {
        cmd.stderr(Stdio::null());
        cmd.stdout(Stdio::null());
    }
    let status = cmd.spawn()?.wait().await?;
    if !status.success() {
        bail!("Build failed.");
    }
    trace!("package compiled");

    let mut executable_path: PathBuf = [
        project_path.clone(),
        target_path.clone(),
        profile.into(),
        binary_name.into(),
    ]
    .iter()
    .collect();
    executable_path.set_extension(std::env::consts::EXE_EXTENSION);

    Ok(BuiltService {
        workspace_path: project_path,
        target_name: binary_name.to_owned(),
        executable_path,
    })
}
