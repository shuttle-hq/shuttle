use std::ffi::OsStr;

use anyhow::Result;

use service::Service;

const ENTRYPOINT_NAME: &'static [u8] = b"_create_service\0";

type CreateService = unsafe extern fn() -> *mut dyn Service;

/// Dynamically load from a `.so` file a value of a type implementing the
/// [`service::Service`] trait. Relies on the `.so` library having an `extern
/// "C"` function called [`ENTRYPOINT_NAME`], likely automatically generated
/// using the [`service::declare_service`] macro.
///
/// Note that included in the return type is an instance of
/// [`libloading::Library`] which must be kept alive in order to use the
/// `Box<dyn Service>` value without causing a segmentation fault.
pub(crate) fn load_service_from_so(so_path: impl AsRef<OsStr>) -> Result<(libloading::Library, Box<dyn Service>)> {
    let (lib, service) = unsafe {
        let lib = libloading::Library::new(so_path)?;

        let entrypoint: libloading::Symbol<CreateService> = lib.get(ENTRYPOINT_NAME)?;
        let raw = entrypoint();

        (lib, Box::from_raw(raw))
    };

    Ok((lib, service))
}
