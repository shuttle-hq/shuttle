use std::ffi::OsStr;

use libloading::{Library, Symbol};
use thiserror::Error;

use crate::Service;

const ENTRYPOINT_SYMBOL_NAME: &[u8] = b"_create_service\0";

type CreateService = unsafe extern "C" fn() -> *mut dyn Service;

#[derive(Debug, Error)]
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
    /// Dynamically load from a `.so` file a value of a type implementing the
    /// [`Service`] trait. Relies on the `.so` library having an ``extern "C"`
    /// function called [`ENTRYPOINT_SYMBOL_NAME`], likely automatically generated
    /// using the [`shuttle_service::declare_service`] macro.
    pub fn from_so_file<P: AsRef<OsStr>>(so_path: P) -> Result<Self, LoaderError> {
        unsafe {
            let lib = Library::new(so_path).map_err(|e| LoaderError::Load(e))?;

            let entrypoint: Symbol<CreateService> = lib
                .get(ENTRYPOINT_SYMBOL_NAME)
                .map_err(|e| LoaderError::GetEntrypoint(e))?;
            let raw = entrypoint();

            Ok(Self {
                service: Box::from_raw(raw),
                so: lib,
            })
        }
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

        // This '.so' is a copy of the rocket/hello-world example with the shuttle macro removed
        #[test]
        fn not_shuttle() {
            let result = Loader::from_so_file("tests/resources/not_shuttle.so");

            assert!(matches!(result, Err(LoaderError::GetEntrypoint(_))));
        }

        // This '.so' is a copy of the rocket/hello-world example
        #[test]
        fn valid() {
            Loader::from_so_file("tests/resources/hello_world.so").unwrap();
        }
    }
}
