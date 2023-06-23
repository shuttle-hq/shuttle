use std::fs::read_to_string;
use std::io::BufRead;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context};
use cargo_metadata::Message;
use cargo_metadata::{Package, Target};
use crossbeam_channel::Sender;
use shuttle_common::project::ProjectName;
use tracing::{debug, error, trace};

use crate::{NEXT_NAME, RUNTIME_NAME};

#[derive(Clone, Debug, Eq, PartialEq)]
/// This represents a compiled alpha or shuttle-next service.
pub struct BuiltService {
    pub executable_path: PathBuf,
    pub is_wasm: bool,
    pub package_name: String,
    pub working_directory: PathBuf,
    pub manifest_path: PathBuf,
}

impl BuiltService {
    pub fn new(
        executable_path: PathBuf,
        is_wasm: bool,
        package_name: String,
        working_directory: PathBuf,
        manifest_path: PathBuf,
    ) -> Self {
        Self {
            executable_path,
            is_wasm,
            package_name,
            working_directory,
            manifest_path,
        }
    }

    /// Try to get the service name of a crate from Shuttle.toml in the crate root, if it doesn't
    /// exist get it from the Cargo.toml package name of the crate.
    pub fn service_name(&self) -> anyhow::Result<ProjectName> {
        let shuttle_toml_path = self.working_directory.join("Shuttle.toml");

        match extract_shuttle_toml_name(shuttle_toml_path) {
            Ok(service_name) => Ok(service_name.parse()?),
            Err(error) => {
                debug!(?error, "failed to get service name from Shuttle.toml");

                // Couldn't get name from Shuttle.toml, use package name instead.
                Ok(self.package_name.parse()?)
            }
        }
    }
}

fn extract_shuttle_toml_name(path: PathBuf) -> anyhow::Result<String> {
    let shuttle_toml = read_to_string(path).context("Shuttle.toml not found")?;

    let toml: toml::Value =
        toml::from_str(&shuttle_toml).context("failed to parse Shuttle.toml")?;

    let name = toml
        .get("name")
        .context("couldn't find `name` key in Shuttle.toml")?
        .as_str()
        .context("`name` key in Shuttle.toml must be a string")?
        .to_string();

    Ok(name)
}

/// Given a project directory path, builds the crate
pub async fn build_workspace(
    project_path: &Path,
    release_mode: bool,
    tx: Sender<Message>,
    deployment: bool,
) -> anyhow::Result<Vec<BuiltService>> {
    let project_path = project_path.to_owned();

    let manifest_path = project_path.join("Cargo.toml");

    if !manifest_path.exists() {
        return Err(anyhow!("failed to read the Shuttle project manifest"));
    }
    let metadata = cargo_metadata::MetadataCommand::new()
        .manifest_path(&manifest_path)
        .exec()?;
    trace!("Cargo metadata parsed");

    let mut alpha_packages = Vec::new();
    let mut next_packages = Vec::new();

    for member in metadata.workspace_packages() {
        if is_next(member) {
            ensure_cdylib(member)?;
            next_packages.push(member);
        } else if is_alpha(member) {
            ensure_binary(member)?;
            alpha_packages.push(member);
        }
    }

    let mut runtimes = Vec::new();

    if !alpha_packages.is_empty() {
        let mut service = compile(
            alpha_packages,
            release_mode,
            false,
            project_path.clone(),
            metadata.target_directory.clone(),
            deployment,
            tx.clone(),
        )
        .await?;
        trace!("alpha packages compiled");

        runtimes.append(&mut service);
    }

    if !next_packages.is_empty() {
        let mut service = compile(
            next_packages,
            release_mode,
            true,
            project_path,
            metadata.target_directory.clone(),
            deployment,
            tx,
        )
        .await?;
        trace!("next packages compiled");

        runtimes.append(&mut service);
    }

    Ok(runtimes)
}

pub async fn clean_crate(project_path: &Path, release_mode: bool) -> anyhow::Result<Vec<String>> {
    let project_path = project_path.to_owned();
    let manifest_path = project_path.join("Cargo.toml");
    if !manifest_path.exists() {
        return Err(anyhow!("failed to read the Shuttle project manifest"));
    }
    let mut profile = "dev";
    if release_mode {
        profile = "release";
    }
    let output = tokio::process::Command::new("cargo")
        .arg("clean")
        .arg("--manifest-path")
        .arg(manifest_path.to_str().unwrap())
        .arg("--profile")
        .arg(profile)
        .output()
        .await
        .unwrap();

    if output.status.success() {
        let lines = vec![
            String::from_utf8(output.clone().stderr)?,
            String::from_utf8(output.stdout)?,
        ];
        Ok(lines)
    } else {
        Err(anyhow!(
            "cargo clean failed with exit code {} and error {}",
            output.clone().status.to_string(),
            String::from_utf8(output.stderr)?
        ))
    }
}

fn is_next(package: &Package) -> bool {
    package
        .dependencies
        .iter()
        .any(|dependency| dependency.name == NEXT_NAME)
}

fn is_alpha(package: &Package) -> bool {
    package
        .dependencies
        .iter()
        .any(|dependency| dependency.name == RUNTIME_NAME)
}

/// Make sure the project is a binary for alpha projects.
fn ensure_binary(package: &Package) -> anyhow::Result<()> {
    if package.targets.iter().any(|target| target.is_bin()) {
        Ok(())
    } else {
        bail!("Your Shuttle project must be a binary.")
    }
}

/// Make sure "cdylib" is set for shuttle-next projects, else set it if possible.
fn ensure_cdylib(package: &Package) -> anyhow::Result<()> {
    if package.targets.iter().any(is_cdylib) {
        Ok(())
    } else {
        bail!("Your Shuttle next project must be a library. Please add `[lib]` to your Cargo.toml file.")
    }
}

fn is_cdylib(target: &Target) -> bool {
    target.kind.iter().any(|kind| kind == "cdylib")
}

async fn compile(
    packages: Vec<&Package>,
    release_mode: bool,
    wasm: bool,
    project_path: PathBuf,
    target_path: impl Into<PathBuf>,
    deployment: bool,
    tx: Sender<Message>,
) -> anyhow::Result<Vec<BuiltService>> {
    let manifest_path = project_path.join("Cargo.toml");
    let target_path = target_path.into();

    let mut cargo = tokio::process::Command::new("cargo");

    let (reader, writer) = os_pipe::pipe()?;
    let writer_clone = writer.try_clone()?;
    cargo.stdout(writer);
    cargo.stderr(writer_clone);

    cargo.arg("build").arg("--manifest-path").arg(manifest_path);

    if deployment {
        cargo.arg("-j").arg(4.to_string());
    }

    for package in packages.clone() {
        cargo.arg("--package").arg(package.name.clone());
    }

    let mut profile = "debug";

    if release_mode {
        profile = "release";
        cargo.arg("--profile").arg("release");
    } else {
        cargo.arg("--profile").arg("dev");
    }

    if wasm {
        cargo.arg("--target").arg("wasm32-wasi");
    }

    let mut handle = cargo.spawn()?;

    tokio::task::spawn_blocking(move || {
        let reader = std::io::BufReader::new(reader);
        for line in reader.lines() {
            if let Ok(line) = line {
                if let Err(error) = tx.send(Message::TextLine(line)) {
                    error!("failed to send cargo message on channel: {error}");
                };
            } else {
                error!("Failed to read Cargo log messages");
            };
        }
    });

    let command = handle.wait().await?;

    if !command.success() {
        bail!("Build failed. Is the Shuttle runtime missing?");
    }

    let mut outputs = Vec::new();

    for package in packages {
        if wasm {
            let mut path: PathBuf = [
                project_path.clone(),
                target_path.clone(),
                "wasm32-wasi".into(),
                profile.into(),
                #[allow(clippy::single_char_pattern)]
                package.clone().name.replace("-", "_").into(),
            ]
            .iter()
            .collect();
            path.set_extension("wasm");

            let mut working_directory = package.clone().manifest_path.into_std_path_buf();
            working_directory.pop();

            let output = BuiltService::new(
                path.clone(),
                true,
                package.clone().name,
                working_directory,
                package.clone().manifest_path.into_std_path_buf(),
            );

            outputs.push(output);
        } else {
            let mut path: PathBuf = [
                project_path.clone(),
                target_path.clone(),
                profile.into(),
                package.clone().name.into(),
            ]
            .iter()
            .collect();
            path.set_extension(std::env::consts::EXE_EXTENSION);

            let mut working_directory = package.clone().manifest_path.into_std_path_buf();
            working_directory.pop();

            let output = BuiltService::new(
                path.clone(),
                false,
                package.clone().name,
                working_directory,
                package.clone().manifest_path.into_std_path_buf(),
            );

            outputs.push(output);
        }
    }

    Ok(outputs)
}
