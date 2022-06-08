use std::ffi::OsStr;
use std::net::SocketAddr;
use std::panic::AssertUnwindSafe;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context};
use cargo::core::compiler::CompileMode;
use cargo::core::{Shell, Verbosity, Workspace};
use cargo::ops::{compile, CompileOptions};
use cargo::util::homedir;
use cargo::Config;
use libloading::{Library, Symbol};
use shuttle_common::DeploymentId;
use thiserror::Error as ThisError;
use tokio::sync::mpsc::UnboundedSender;

use futures::FutureExt;

use crate::{
    logger::{Log, Logger},
    Error, Factory, ServeHandle, Service,
};

const ENTRYPOINT_SYMBOL_NAME: &[u8] = b"_create_service\0";

type CreateService = unsafe extern "C" fn() -> *mut dyn Service;

#[derive(Debug, ThisError)]
pub enum LoaderError {
    #[error("failed to load library")]
    Load(libloading::Error),
    #[error("failed to find the shuttle entrypoint. Did you use the provided shuttle macros?")]
    GetEntrypoint(libloading::Error),
}

pub struct Loader {
    service: Box<dyn Service>,
    so: Library,
}

impl Loader {
    #[allow(dead_code)]
    pub fn new(service: Box<dyn Service>, so: Library) -> Self {
        Loader { service, so }
    }

    /// Dynamically load from a `.so` file a value of a type implementing the
    /// [`Service`] trait. Relies on the `.so` library having an ``extern "C"`
    /// function called [`ENTRYPOINT_SYMBOL_NAME`], likely automatically generated
    /// using the [`shuttle_service::declare_service`] macro.
    pub fn from_so_file<P: AsRef<OsStr>>(so_path: P) -> Result<Self, LoaderError> {
        unsafe {
            let lib = Library::new(so_path).map_err(LoaderError::Load)?;

            let entrypoint: Symbol<CreateService> = lib
                .get(ENTRYPOINT_SYMBOL_NAME)
                .map_err(LoaderError::GetEntrypoint)?;
            let raw = entrypoint();

            Ok(Self {
                service: Box::from_raw(raw),
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
        let mut service = self.service;
        let logger = Box::new(Logger::new(tx, deployment_id));

        AssertUnwindSafe(service.build(factory, logger))
            .catch_unwind()
            .await
            .unwrap_or(Err(Error::BuildPanic))?;

        // channel used by task spawned below to indicate whether or not panic
        // occurred in `service.bind` call
        let (send, recv) = tokio::sync::oneshot::channel();

        // Start service on this side of the FFI
        let handle = tokio::spawn(async move {
            let bound = AssertUnwindSafe(async { service.bind(addr) })
                .catch_unwind()
                .await;

            send.send(bound.is_ok()).unwrap();

            if let Ok(b) = bound {
                b?.await?
            } else {
                Err(anyhow!("panic in `Service::bound`"))
            }
        });

        if matches!(recv.await, Ok(true)) {
            Ok((handle, self.so))
        } else {
            Err(Error::BindPanic)
        }
    }
}

/// Given a project directory path, builds the crate
pub fn build_crate(project_path: &Path, buf: Box<dyn std::io::Write>) -> anyhow::Result<PathBuf> {
    let mut shell = Shell::from_write(buf);
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

    let ws = Workspace::new(&manifest_path, &config)?;

    if let Some(profiles) = ws.profiles() {
        for profile in profiles.get_all().values() {
            if profile.panic.as_deref() == Some("abort") {
                return Err(anyhow!("a Shuttle project cannot have panics that abort. Please ensure your Cargo.toml does not contain `panic = \"abort\"` for any profiles"));
            }
        }
    }

    let opts = CompileOptions::new(&config, CompileMode::Build)?;
    let compilation = compile(&ws, &opts)?;

    if compilation.cdylibs.is_empty() {
        return Err(anyhow!("a cdylib was not created. Try adding the following to the Cargo.toml of the service:\n[lib]\ncrate-type = [\"cdylib\"]\n"));
    }

    Ok(compilation.cdylibs[0].path.clone())
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
