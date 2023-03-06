use std::any::Any;
use std::ffi::OsStr;
use std::net::SocketAddr;
use std::panic::AssertUnwindSafe;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context};
use cargo::core::compiler::{CompileKind, CompileMode, CompileTarget, MessageFormat};
use cargo::core::{Manifest, PackageId, Shell, Summary, Verbosity, Workspace};
use cargo::ops::{clean, compile, CleanOptions, CompileOptions};
use cargo::util::interning::InternedString;
use cargo::util::{homedir, ToSemver};
use cargo::Config;
use cargo_metadata::Message;
use crossbeam_channel::Sender;
use libloading::{Library, Symbol};
use pipe::PipeWriter;
use thiserror::Error as ThisError;
use tracing::{error, trace};

use futures::FutureExt;
use uuid::Uuid;

use crate::error::CustomError;
use crate::{logger, Bootstrapper, NAME, NEXT_NAME, VERSION};
use crate::{Error, Factory, ServeHandle};

const ENTRYPOINT_SYMBOL_NAME: &[u8] = b"_create_service\0";

type CreateService = unsafe extern "C" fn() -> *mut Bootstrapper;

#[derive(Debug, ThisError)]
pub enum LoaderError {
    #[error("failed to load library: {0}")]
    Load(libloading::Error),
    #[error("failed to find the shuttle entrypoint. Did you use the provided shuttle macros?")]
    GetEntrypoint(libloading::Error),
}

pub type LoadedService = (ServeHandle, Library);

pub struct Loader {
    bootstrapper: Bootstrapper,
    so: Library,
}

impl Loader {
    /// Dynamically load from a `.so` file a value of a type implementing the
    /// [`Service`][crate::Service] trait. Relies on the `.so` library having an `extern "C"`
    /// function called `ENTRYPOINT_SYMBOL_NAME`, likely automatically generated
    /// using the [`shuttle_service::main`][crate::main] macro.
    pub fn from_so_file<P: AsRef<OsStr>>(so_path: P) -> Result<Self, LoaderError> {
        trace!(so_path = so_path.as_ref().to_str(), "loading .so path");
        unsafe {
            let lib = Library::new(so_path).map_err(LoaderError::Load)?;

            let entrypoint: Symbol<CreateService> = lib
                .get(ENTRYPOINT_SYMBOL_NAME)
                .map_err(LoaderError::GetEntrypoint)?;
            let raw = entrypoint();

            Ok(Self {
                bootstrapper: *Box::from_raw(raw),
                so: lib,
            })
        }
    }

    pub async fn load(
        self,
        factory: &mut dyn Factory,
        addr: SocketAddr,
        logger: logger::Logger,
    ) -> Result<LoadedService, Error> {
        trace!("loading service");

        let mut bootstrapper = self.bootstrapper;

        AssertUnwindSafe(bootstrapper.bootstrap(factory, logger))
            .catch_unwind()
            .await
            .map_err(|e| Error::BuildPanic(map_any_to_panic_string(e)))??;

        trace!("bootstrapping done");

        // Start service on this side of the FFI
        let handle = tokio::spawn(async move {
            bootstrapper.into_handle(addr)?.await.map_err(|e| {
                if e.is_panic() {
                    let mes = e.into_panic();

                    Error::BindPanic(map_any_to_panic_string(mes))
                } else {
                    Error::Custom(CustomError::new(e))
                }
            })?
        });

        trace!("creating handle done");

        Ok((handle, self.so))
    }
}

/// How to run/build the project
pub enum Runtime {
    Next(PathBuf),
    Legacy(PathBuf),
}

/// Given a project directory path, builds the crate
pub async fn build_crate(
    deployment_id: Uuid,
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

    let current = ws.current_mut().map_err(|_| anyhow!("A Shuttle project cannot have a virtual manifest file - please ensure your Cargo.toml file specifies it as a library."))?;

    let summary = current.manifest_mut().summary_mut();
    make_name_unique(summary, deployment_id);

    let is_next = is_next(summary);
    if !is_next {
        check_version(summary)?;
    }
    check_no_panic(&ws)?;

    let opts = get_compile_options(&config, release_mode, is_next)?;
    let compilation = compile(&ws, &opts);

    let path = compilation?.binaries[0].path.clone();
    Ok(if is_next {
        Runtime::Next(path)
    } else {
        Runtime::Legacy(path)
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

/// Make sure "cdylib" is set, else set it if possible
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
        Err(anyhow!(
            "Your Shuttle project must be a library. Please add `[lib]` to your Cargo.toml file."
        ))
    }
}

/// Ensure name is unique. Without this `tracing`/`log` crashes because the global subscriber is somehow "already set"
// TODO: remove this when getting rid of the FFI
fn make_name_unique(summary: &mut Summary, deployment_id: Uuid) {
    let old_package_id = summary.package_id();
    *summary = summary.clone().override_id(
        PackageId::new(
            format!("{}-{deployment_id}", old_package_id.name()),
            old_package_id.version(),
            old_package_id.source_id(),
        )
        .unwrap(),
    );
}

fn is_next(summary: &Summary) -> bool {
    summary
        .dependencies()
        .iter()
        .any(|dependency| dependency.package_name() == NEXT_NAME)
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

fn map_any_to_panic_string(a: Box<dyn Any>) -> String {
    a.downcast_ref::<&str>()
        .map(|x| x.to_string())
        .unwrap_or_else(|| "<no panic message>".to_string())
}

#[cfg(test)]
mod tests {
    mod from_so_file {
        use crate::loader::{Loader, LoaderError};

        #[test]
        fn invalid() {
            let result = Loader::from_so_file("invalid.so");

            assert!(matches!(result, Err(LoaderError::Load(_))));
        }
    }
}
