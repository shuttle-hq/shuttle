use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context};
use cargo::core::compiler::{CompileKind, CompileMode, CompileTarget, MessageFormat};
use cargo::core::{Manifest, Shell, Summary, Verbosity, Workspace};
use cargo::ops::{clean, compile, CleanOptions, CompileOptions};
use cargo::util::interning::InternedString;
use cargo::util::{homedir, ToSemver};
use cargo::Config;
use cargo_metadata::Message;
use crossbeam_channel::Sender;
use pipe::PipeWriter;
use tracing::{error, trace};

use crate::{NAME, NEXT_NAME, VERSION};

/// How to run/build the project
pub enum Runtime {
    Next(PathBuf),
    Legacy(PathBuf),
}

/// Given a project directory path, builds the crate
pub async fn build_crate(
    project_path: &Path,
    release_mode: bool,
    tx: Sender<Message>,
) -> anyhow::Result<Runtime> {
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
    let mut ws = Workspace::new(&manifest_path, &config)?;

    let current = ws.current_mut().map_err(|_| anyhow!("A Shuttle project cannot have a virtual manifest file - please ensure the `package` table is present in your Cargo.toml file."))?;

    let summary = current.manifest_mut().summary_mut();
    let is_next = is_next(summary);

    if !is_next {
        check_version(summary)?;
        ensure_binary(current.manifest())?;
    } else {
        ensure_cdylib(current.manifest_mut())?;
    }

    check_no_panic(&ws)?;

    let opts = get_compile_options(&config, release_mode, is_next)?;
    let compilation = compile(&ws, &opts)?;

    Ok(if is_next {
        Runtime::Next(compilation.cdylibs[0].path.clone())
    } else {
        Runtime::Legacy(compilation.binaries[0].path.clone())
    })
}

pub fn clean_crate(project_path: &Path, release_mode: bool) -> anyhow::Result<Vec<String>> {
    let (read, write) = pipe::pipe();
    let project_path = project_path.to_owned();

    tokio::task::spawn_blocking(move || {
        let config = get_config(write).unwrap();
        let manifest_path = project_path.join("Cargo.toml");
        let ws = Workspace::new(&manifest_path, &config).unwrap();

        let requested_profile = if release_mode {
            InternedString::new("release")
        } else {
            InternedString::new("dev")
        };

        let opts = CleanOptions {
            config: &config,
            spec: Vec::new(),
            targets: Vec::new(),
            requested_profile,
            profile_specified: true,
            doc: false,
        };

        clean(&ws, &opts).unwrap();
    });

    let mut lines = Vec::new();

    for message in Message::parse_stream(read) {
        trace!(?message, "parsed cargo message");
        match message {
            Ok(Message::TextLine(line)) => {
                lines.push(line);
            }
            Ok(_) => {}
            Err(error) => {
                error!("failed to parse cargo message: {error}");
            }
        }
    }

    Ok(lines)
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

    Ok(opts)
}

fn is_next(summary: &Summary) -> bool {
    summary
        .dependencies()
        .iter()
        .any(|dependency| dependency.package_name() == NEXT_NAME)
}

/// Make sure the project is a binary for legacy projects.
fn ensure_binary(manifest: &Manifest) -> anyhow::Result<()> {
    if manifest.targets().iter().any(|target| target.is_bin()) {
        Ok(())
    } else {
        bail!("Your Shuttle project must be a binary.")
    }
}

/// Make sure "cdylib" is set for shuttle-next projects, else set it if possible.
fn ensure_cdylib(manifest: &mut Manifest) -> anyhow::Result<()> {
    if let Some(target) = manifest
        .targets_mut()
        .iter_mut()
        .find(|target| target.is_lib())
    {
        if !target.is_cdylib() {
            *target = cargo::core::manifest::Target::lib_target(
                target.name(),
                vec![cargo::core::compiler::CrateType::Cdylib],
                target.src_path().path().unwrap().to_path_buf(),
                target.edition(),
            );
        }

        Ok(())
    } else {
        bail!("Your Shuttle project must be a library. Please add `[lib]` to your Cargo.toml file.")
    }
}

/// Check that the crate being build is compatible with this version of loader
fn check_version(summary: &Summary) -> anyhow::Result<()> {
    let valid_version = VERSION.to_semver().unwrap();

    let version_req = if let Some(shuttle) = summary
        .dependencies()
        .iter()
        .find(|dependency| dependency.package_name() == NAME)
    {
        shuttle.version_req()
    } else {
        return Err(anyhow!("this crate does not use the shuttle service"));
    };

    if version_req.matches(&valid_version) {
        Ok(())
    } else {
        Err(anyhow!(
            "the version of `shuttle-service` specified as a dependency to this service ({version_req}) is not supported by this project instance ({valid_version}); try updating `shuttle-service` to '{valid_version}' or update the project instance using `cargo shuttle project rm` and `cargo shuttle project new`"
        ))
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
