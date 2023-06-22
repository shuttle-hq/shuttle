use std::{fmt::Debug, path::PathBuf, str::FromStr};

use tonic::transport::{Endpoint, Uri};

use crate::args::args;

args! {
    pub struct Args {
        "--port" => pub port: u16,
        "--provisioner-address" => #[arg(default_value = "http://localhost:3000")] pub provisioner_address: Endpoint,
        "--storage-manager-type" => pub storage_manager_type: StorageManagerType,
        "--storage-manager-path" => pub storage_manager_path: PathBuf,
        "--auth-uri" => #[arg(default_value = "http://127.0.0.1:8008")] pub auth_uri: Uri,
        "--logger-uri" => #[arg(default_value = "http://127.0.0.1:8009")] pub logger_uri: Uri,
    }
}

#[derive(Clone, Debug)]
pub enum StorageManagerType {
    Artifacts,
    WorkingDir,
}

impl FromStr for StorageManagerType {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "artifacts" => Ok(StorageManagerType::Artifacts),
            "working-dir" => Ok(StorageManagerType::WorkingDir),
            _ => Err(()),
        }
    }
}
