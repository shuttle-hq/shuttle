use std::{ffi::OsStr, net::SocketAddr, sync::Arc};

use libloading::{Library, Symbol};
use thiserror::Error as ThisError;
use tokio::task::JoinHandle;

use crate::{Error, Factory, Service};

const ENTRYPOINT_SYMBOL_NAME: &[u8] = b"_create_service\0";

type CreateService = unsafe extern "C" fn() -> *mut dyn Service;

pub type ServeHandle = JoinHandle<Result<(), Error>>;

#[derive(Debug, ThisError)]
pub enum LoaderError {
    #[error("failed to load library")]
    Load(libloading::Error),
    #[error("failed to find the shuttle entrypoint. Did you use the provided shuttle macros?")]
    GetEntrypoint(libloading::Error),
}

pub struct Loader {
    pub service: Arc<std::sync::Mutex<Box<dyn Service>>>,
    pub so: Library,
    pub thread_handle: Option<ServeHandle>,
}

impl Loader {
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
                service: Arc::new(std::sync::Mutex::new(Box::from_raw(raw))),
                so: lib,
                thread_handle: None,
            })
        }
    }

    pub fn load(mut self, factory: &mut dyn Factory, addr: SocketAddr) -> Result<Self, Error> {
        // let mut service = self.service;

        self.service.lock().unwrap().build(factory)?;

        let service = Arc::clone(&self.service);

        // We cannot use spawn here since that blocks the api completely. We suspect this is because `bind` makes a blocking call,
        // however that does not completely makes sense as the blocking call is made on another runtime.
        let thread_handle = Some(tokio::task::spawn_blocking(move || {
            service.lock().unwrap().bind(addr)
        }));
        self.thread_handle = thread_handle;

        Ok(self)
    }
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
