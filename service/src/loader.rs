use std::ffi::OsStr;
use std::net::SocketAddr;

use libloading::{
    Library,
    Symbol
};
use thiserror::Error as ThisError;
use tokio::task::JoinHandle;

use crate::{
    Error,
    Factory,
    Service
};

const ENTRYPOINT_SYMBOL_NAME: &[u8] = b"_create_service\0";

type CreateService = unsafe extern "C" fn() -> *mut dyn Service;

pub type ServeHandle = JoinHandle<Result<(), Error>>;

#[derive(Debug, ThisError)]
pub enum LoaderError {
    #[error("failed to load library")]
    Load(libloading::Error),
    #[error("failed to find the shuttle entrypoint. Did you use the provided shuttle macros?")]
    GetEntrypoint(libloading::Error)
}

pub struct Loader {
    service: Box<dyn Service>,
    so: Library
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
                service: Box::from_raw(raw),
                so: lib
            })
        }
    }

    pub fn load(
        self,
        factory: &mut dyn Factory,
        addr: SocketAddr
    ) -> Result<(ServeHandle, Library), Error> {
        let mut service = self.service;

        service.build(factory)?;

        // We cannot use spawn here since that blocks the api completely. We suspect this is because `bind` makes a blocking call,
        // however that does not completely makes sense as the blocking call is made on another runtime.
        let handle = tokio::task::spawn_blocking(move || service.bind(addr));

        Ok((handle, self.so))
    }
}

#[cfg(test)]
mod tests {
    mod from_so_file {
        use crate::loader::{
            Loader,
            LoaderError
        };

        #[test]
        fn invalid() {
            let result = Loader::from_so_file("invalid.so");

            assert!(matches!(result, Err(LoaderError::Load(_))));
        }
    }
}
