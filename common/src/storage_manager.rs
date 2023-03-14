use std::{fs, io, path::PathBuf};

use uuid::Uuid;

pub trait StorageManager: Sync + Send {
    /// Path for a specific service build files
    fn service_build_path(&self, service_name: &str) -> Result<PathBuf, io::Error>;

    /// Path to folder for storing service files
    fn service_storage_path(&self, service_name: &str) -> Result<PathBuf, io::Error>;
}

/// Manager to take care of directories for storing project, services and deployment files for deployer
#[derive(Clone)]
pub struct ArtifactsStorageManager {
    artifacts_path: PathBuf,
}

impl ArtifactsStorageManager {
    pub fn new(artifacts_path: PathBuf) -> Self {
        Self { artifacts_path }
    }

    /// Path of the directory that contains extracted service Cargo projects.
    pub fn builds_path(&self) -> Result<PathBuf, io::Error> {
        let builds_path = self.artifacts_path.join("shuttle-builds");
        fs::create_dir_all(&builds_path)?;

        Ok(builds_path)
    }

    /// The directory in which compiled executables are stored.
    pub fn executables_path(&self) -> Result<PathBuf, io::Error> {
        let executables_path = self.artifacts_path.join("shuttle-executables");
        fs::create_dir_all(&executables_path)?;

        Ok(executables_path)
    }

    /// Path to executable for a service
    pub fn deployment_executable_path(&self, deployment_id: &Uuid) -> Result<PathBuf, io::Error> {
        let executable_path = self.executables_path()?.join(deployment_id.to_string());

        Ok(executable_path)
    }

    /// Path of the directory to store user files
    pub fn storage_path(&self) -> Result<PathBuf, io::Error> {
        let storage_path = self.artifacts_path.join("shuttle-storage");
        fs::create_dir_all(&storage_path)?;

        Ok(storage_path)
    }
}

impl StorageManager for ArtifactsStorageManager {
    fn service_build_path(&self, service_name: &str) -> Result<PathBuf, io::Error> {
        let builds_path = self.builds_path()?.join(service_name);
        fs::create_dir_all(&builds_path)?;

        Ok(builds_path)
    }

    fn service_storage_path(&self, service_name: &str) -> Result<PathBuf, io::Error> {
        let storage_path = self.storage_path()?.join(service_name);
        fs::create_dir_all(&storage_path)?;

        Ok(storage_path)
    }
}

/// Manager to take care of directories for storing project, services and deployment files for the local runner
#[derive(Clone)]
pub struct WorkingDirStorageManager {
    working_dir: PathBuf,
}

impl WorkingDirStorageManager {
    pub fn new(working_dir: PathBuf) -> Self {
        Self { working_dir }
    }
}

impl StorageManager for WorkingDirStorageManager {
    fn service_build_path(&self, _service_name: &str) -> Result<PathBuf, io::Error> {
        Ok(self.working_dir.clone())
    }

    fn service_storage_path(&self, _service_name: &str) -> Result<PathBuf, io::Error> {
        Ok(self.working_dir.clone())
    }
}
