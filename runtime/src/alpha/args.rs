use std::path::PathBuf;

use clap::{Parser, ValueEnum};
use tonic::transport::Endpoint;

#[derive(Parser, Debug)]
#[command(version)]
pub struct Args {
    /// Port to start runtime on
    #[arg(long)]
    pub port: u16,

    /// Address to reach provisioner at
    #[arg(long, default_value = "http://localhost:5000")]
    pub provisioner_address: Endpoint,

    /// Type of storage manager to start
    #[arg(long, value_enum)]
    pub storage_manager_type: StorageManagerType,

    /// Path to use for storage manager
    #[arg(long)]
    pub storage_manager_path: PathBuf,
}

#[derive(Clone, Debug, ValueEnum)]
pub enum StorageManagerType {
    /// Use a deloyer artifacts directory
    Artifacts,

    /// Use a local working directory
    WorkingDir,
}
