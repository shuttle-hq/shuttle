//
// Constants regarding the deployer environment and conventions
//
/// Where executables are moved to in order to persist across deploys, relative to workspace root
pub const EXECUTABLE_DIRNAME: &str = ".shuttle-executables";
/// Where general files will persist across deploys, relative to workspace root. Used by plugins.
pub const STORAGE_DIRNAME: &str = ".shuttle-storage";

pub const API_URL_LOCAL: &str = "http://localhost:8001";
pub const API_URL_PRODUCTION: &str = "https://api.shuttle.rs";
#[cfg(debug_assertions)]
pub const API_URL_DEFAULT: &str = API_URL_LOCAL;
#[cfg(not(debug_assertions))]
pub const API_URL_DEFAULT: &str = API_URL_PRODUCTION;

// Crate names for checking cargo metadata
pub const NEXT_NAME: &str = "shuttle-next";
pub const RUNTIME_NAME: &str = "shuttle-runtime";
