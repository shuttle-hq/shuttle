use std::fs::read_to_string;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context};
use cargo::core::compiler::{CompileKind, CompileMode, CompileTarget, MessageFormat};
use cargo::core::{Shell, Verbosity, Workspace};
use cargo::ops::{self, compile, CleanOptions, CompileOptions};
use cargo::util::homedir;
use cargo::util::interning::InternedString;
use cargo::Config;
use cargo_metadata::Message;
use cargo_metadata::Package;
use crossbeam_channel::Sender;
use pipe::PipeWriter;
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
) -> anyhow::Result<Vec<BuiltService>> {
    let (read, write) = pipe::pipe();
    let project_path = project_path.to_owned();

    // This needs to be on a separate thread, else deployer will block (reason currently unknown :D)
    tokio::task::spawn_blocking(move || {
        trace!("started thread to to capture build output stream");
        for message in Message::parse_stream(read) {
            trace!(?message, "parsed cargo message");
            match message {
                Ok(message) => {
                    if let Err(error) = tx.send(message) {
                        error!("failed to send cargo message on channel: {error}");
                    }
                }
                Err(error) => {
                    error!("failed to parse cargo message: {error}");
                }
            }
        }
    });

    let config = get_config(write)?;
    let manifest_path = project_path.join("Cargo.toml");
    let metadata = cargo_metadata::MetadataCommand::new()
        .manifest_path(&manifest_path)
        .exec()?;
    let ws = Workspace::new(&manifest_path, &config)?;
    check_no_panic(&ws)?;

    let mut alpha_packages = Vec::new();
    let mut next_packages = Vec::new();

    for member in metadata.workspace_packages() {
        if is_next(member) {
            ensure_cdylib(member)?;
            next_packages.push(member.name().to_string());
        } else if is_alpha(member) {
            ensure_binary(member)?;
            alpha_packages.push(member.name().to_string());
        }
    }

    let mut runtimes = Vec::new();

    if !alpha_packages.is_empty() {
        let opts = get_compile_options(&config, alpha_packages, release_mode, false)?;
        let compilation = compile(&ws, &opts)?;

        let mut alpha_binaries = compilation
            .binaries
            .iter()
            .map(|binary| {
                BuiltService::new(
                    binary.path.clone(),
                    false,
                    binary.unit.pkg.name().to_string(),
                    binary.unit.pkg.root().to_path_buf(),
                    binary.unit.pkg.manifest_path().to_path_buf(),
                )
            })
            .collect();

        runtimes.append(&mut alpha_binaries);
    }

    if !next_packages.is_empty() {
        let opts = get_compile_options(&config, next_packages, release_mode, true)?;
        let compilation = compile(&ws, &opts)?;

        let mut next_libraries = compilation
            .cdylibs
            .iter()
            .map(|binary| {
                BuiltService::new(
                    binary.path.clone(),
                    true,
                    binary.unit.pkg.name().to_string(),
                    binary.unit.pkg.root().to_path_buf(),
                    binary.unit.pkg.manifest_path().to_path_buf(),
                )
            })
            .collect();

        runtimes.append(&mut next_libraries);
    }

    Ok(runtimes)
}

pub fn clean_crate(project_path: &Path, release_mode: bool) -> anyhow::Result<Vec<String>> {
    let project_path = project_path.to_owned();
    let manifest_path = project_path.join("Cargo.toml");
    let mut profile = "dev";
    if release_mode {
        profile = "release";
    }

    tokio::task::spawn_blocking(move || {
        let output = std::process::Command::new("cargo")
            .arg("clean")
            .arg("--manifest-path")
            .arg(manifest_path.to_string())
            .arg("--profile")
            .arg(profile)
            .output()?;
    });

    if output.clone().status.success() {
        let mut lines = Vec::new();

        lines.push(String::from_utf8(output.clone().stdout).unwrap());

        lines.push(String::from_utf8(output.clone().stderr).unwrap());

        Ok(lines)
    } else {
        error!("cargo clean failed.");
    }
}

/// Get the default compile config with output redirected to writer
pub fn get_config(writer: PipeWriter) -> anyhow::Result<Config> {
    let mut shell = Shell::from_write(Box::new(writer));
    shell.set_verbosity(Verbosity::Normal);
    let cwd = std::env::current_dir()
        .with_context(|| "couldn't get the current directory of the process")?;
    let homedir = homedir(&cwd).ok_or_else(|| {
        anyhow!(
            "Cargo couldn't find your home directory. \
                 This probably means that $HOME was not set."
        )
    })?;

    Ok(Config::new(shell, cwd, homedir))
}

/// Get options to compile in build mode
fn get_compile_options(
    config: &Config,
    packages: Vec<String>,
    release_mode: bool,
    wasm: bool,
) -> anyhow::Result<CompileOptions> {
    let mut opts = CompileOptions::new(config, CompileMode::Build)?;
    opts.build_config.message_format = MessageFormat::Json {
        render_diagnostics: false,
        short: false,
        ansi: false,
    };

    opts.build_config.requested_profile = if release_mode {
        InternedString::new("release")
    } else {
        InternedString::new("dev")
    };

    // This sets the max workers for cargo build to 4 for release mode (aka deployment),
    // but leaves it as default (num cpus) for local runs
    if release_mode {
        opts.build_config.jobs = 4
    };

    opts.build_config.requested_kinds = vec![if wasm {
        CompileKind::Target(CompileTarget::new("wasm32-wasi")?)
    } else {
        CompileKind::Host
    }];

    opts.spec = ops::Packages::Packages(packages);

    Ok(opts)
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
    if package.targets.iter().any(|target| target.is_lib()) {
        Ok(())
    } else {
        bail!("Your Shuttle next project must be a library. Please add `[lib]` to your Cargo.toml file.")
    }
}

/// Ensure `panic = "abort"` is not set:
fn check_no_panic(ws: &Workspace) -> anyhow::Result<()> {
    if let Some(profiles) = ws.profiles() {
        for profile in profiles.get_all().values() {
            if profile.panic.as_deref() == Some("abort") {
                return Err(anyhow!("Your Shuttle project cannot have panics that abort. Please ensure your Cargo.toml does not contain `panic = \"abort\"` for any profiles."));
            }
        }
    }

    Ok(())
}
