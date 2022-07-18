use std::any::Any;
use std::ffi::OsStr;
use std::net::SocketAddr;
use std::panic::AssertUnwindSafe;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context};
use cargo::core::compiler::{CompileMode, MessageFormat};
use cargo::core::{Shell, Verbosity, Workspace};
use cargo::ops::{compile, CompileOptions};
use cargo::util::homedir;
use cargo::Config;
use cargo_metadata::Message;
use libloading::{Library, Symbol};
use log::trace;
use shuttle_common::DeploymentId;
use thiserror::Error as ThisError;
use tokio::sync::mpsc::UnboundedSender;

use futures::FutureExt;

use crate::error::CustomError;
use crate::Bootstrapper;
use crate::{
    logger::{Log, Logger},
    Error, Factory, ServeHandle,
};

const ENTRYPOINT_SYMBOL_NAME: &[u8] = b"_create_service\0";

type CreateService = unsafe extern "C" fn() -> *mut Bootstrapper;

#[derive(Debug, ThisError)]
pub enum LoaderError {
    #[error("failed to load library: {0}")]
    Load(libloading::Error),
    #[error("failed to find the shuttle entrypoint. Did you use the provided shuttle macros?")]
    GetEntrypoint(libloading::Error),
}

pub struct Loader {
    bootstrapper: Bootstrapper,
    so: Library,
}

impl Loader {
    /// Dynamically load from a `.so` file a value of a type implementing the
    /// [`Service`] trait. Relies on the `.so` library having an ``extern "C"`
    /// function called [`ENTRYPOINT_SYMBOL_NAME`], likely automatically generated
    /// using the [`shuttle_service::main`] macro.
    pub fn from_so_file<P: AsRef<OsStr>>(so_path: P) -> Result<Self, LoaderError> {
        trace!("loading {:?}", so_path.as_ref().to_str());
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
        tx: UnboundedSender<Log>,
        deployment_id: DeploymentId,
    ) -> Result<(ServeHandle, Library), Error> {
        let mut bootstrapper = self.bootstrapper;
        let logger = Box::new(Logger::new(tx, deployment_id));

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

/// Given a project directory path, builds the crate
pub async fn build_crate(
    project_path: &Path,
    tx: UnboundedSender<Message>,
) -> anyhow::Result<PathBuf> {
    let (read, write) = pipe::pipe();
    let project_path = project_path.to_owned();

    let handle = tokio::spawn(async move {
        let mut shell = Shell::from_write(Box::new(write));
        shell.set_verbosity(Verbosity::Normal);
        let cwd = std::env::current_dir()
            .with_context(|| "couldn't get the current directory of the process")?;
        let homedir = homedir(&cwd).ok_or_else(|| {
            anyhow!(
                "Cargo couldn't find your home directory. \
                 This probably means that $HOME was not set."
            )
        })?;

        let config = Config::new(shell, cwd, homedir);
        let manifest_path = project_path.join("Cargo.toml");

        let mut ws = Workspace::new(&manifest_path, &config)?;

        // Ensure a 'cdylib' will be built:

        let current = ws.current_mut().map_err(|_| anyhow!("A Shuttle project cannot have a virtual manifest file - please ensure your Cargo.toml file specifies it as a library."))?;
        if let Some(target) = current
            .manifest_mut()
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
        } else {
            return Err(anyhow!(
            "Your Shuttle project must be a library. Please add `[lib]` to your Cargo.toml file."
        ));
        }

        // Ensure `panic = "abort"` is not set:

        if let Some(profiles) = ws.profiles() {
            for profile in profiles.get_all().values() {
                if profile.panic.as_deref() == Some("abort") {
                    return Err(anyhow!("Your Shuttle project cannot have panics that abort. Please ensure your Cargo.toml does not contain `panic = \"abort\"` for any profiles."));
                }
            }
        }

        let mut opts = CompileOptions::new(&config, CompileMode::Build)?;
        opts.build_config.message_format = MessageFormat::Json {
            render_diagnostics: false,
            short: false,
            ansi: false,
        };

        let compilation = compile(&ws, &opts);

        Ok(compilation?.cdylibs[0].path.clone())
    });

    // This needs to be on a separate thread, else deployer will block (reason currently unknown :D)
    tokio::spawn(async move {
        for message in Message::parse_stream(read) {
            let message = message.expect("to parse cargo message");
            tx.send(message).expect("to send cargo message on channel");
        }
    });

    handle.await?
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
