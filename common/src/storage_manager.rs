use std::{fs, io, path::PathBuf};

use uuid::Uuid;

pub trait StorageManager: Clone + Sync + Send {
    /// Path for a specific service build files
    fn service_build_path<S: AsRef<str>>(&self, service_name: S) -> Result<PathBuf, io::Error>;

    /// Path to folder for storing deployment files
    fn deployment_storage_path<S: AsRef<str>>(
        &self,
        service_name: S,
        deployment_id: &Uuid,
    ) -> Result<PathBuf, io::Error>;
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

    /// The directory in which compiled '.so' files are stored.
    pub fn libraries_path(&self) -> Result<PathBuf, io::Error> {
        let libs_path = self.artifacts_path.join("shuttle-libs");
        fs::create_dir_all(&libs_path)?;

        Ok(libs_path)
    }

    /// Path to `.so` for a service
    pub fn deployment_library_path(&self, deployment_id: &Uuid) -> Result<PathBuf, io::Error> {
        let library_path = self.libraries_path()?.join(deployment_id.to_string());

        Ok(library_path)
    }

    /// Path of the directory to store user files
    pub fn storage_path(&self) -> Result<PathBuf, io::Error> {
        let storage_path = self.artifacts_path.join("shuttle-storage");
        fs::create_dir_all(&storage_path)?;

        Ok(storage_path)
    }
}

impl StorageManager for ArtifactsStorageManager {
    fn service_build_path<S: AsRef<str>>(&self, service_name: S) -> Result<PathBuf, io::Error> {
        let builds_path = self.builds_path()?.join(service_name.as_ref());
        fs::create_dir_all(&builds_path)?;

        Ok(builds_path)
    }

    fn deployment_storage_path<S: AsRef<str>>(
        &self,
        service_name: S,
        deployment_id: &Uuid,
    ) -> Result<PathBuf, io::Error> {
        let storage_path = self
            .storage_path()?
            .join(service_name.as_ref())
            .join(deployment_id.to_string());
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
    fn service_build_path<S: AsRef<str>>(&self, _service_name: S) -> Result<PathBuf, io::Error> {
        Ok(self.working_dir.clone())
    }

    fn deployment_storage_path<S: AsRef<str>>(
        &self,
        _service_name: S,
        _deployment_id: &Uuid,
    ) -> Result<PathBuf, io::Error> {
        Ok(self.working_dir.clone())
    }
}
