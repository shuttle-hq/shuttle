use std::{fs, path::PathBuf};

use uuid::Uuid;

/// Manager to take care of directories for storing project, services and deployment files
#[derive(Clone)]
pub struct StorageManager {
    artifacts_path: PathBuf,
}

impl StorageManager {
    pub fn new(artifacts_path: PathBuf) -> Self {
        Self { artifacts_path }
    }

    /// Path of the directory that contains extracted service Cargo projects.
    pub fn builds_path(&self) -> PathBuf {
        let builds_path = self.artifacts_path.join("shuttle-builds");
        fs::create_dir_all(&builds_path).expect("could not create builds directory");

        builds_path
    }

    /// Path for a specific service
    pub fn service_build_path<S: AsRef<str>>(&self, service_name: S) -> PathBuf {
        let builds_path = self.builds_path().join(service_name.as_ref());
        fs::create_dir_all(&builds_path).expect("could not create service builds directory");

        builds_path
    }

    /// The directory in which compiled '.so' files are stored.
    pub fn libraries_path(&self) -> PathBuf {
        let libs_path = self.artifacts_path.join("shuttle-libs");
        fs::create_dir_all(&libs_path).expect("could not create libs directory");

        libs_path
    }

    /// Path to `.so` for a service
    pub fn deployment_library_path(&self, deployment_id: &Uuid) -> PathBuf {
        self.libraries_path().join(deployment_id.to_string())
    }

    /// Path of the directory to store user files
    pub fn storage_path(&self) -> PathBuf {
        let storage_path = self.artifacts_path.join("shuttle-storage");
        fs::create_dir_all(&storage_path).expect("could not create storage directory");

        storage_path
    }

    /// Path to folder for storing deployment files
    pub fn deployment_storage_path<S: AsRef<str>>(
        &self,
        service_name: S,
        deployment_id: &Uuid,
    ) -> PathBuf {
        let storage_path = self
            .storage_path()
            .join(service_name.as_ref())
            .join(deployment_id.to_string());
        fs::create_dir_all(&storage_path).expect("could not create deployment storage directory");

        storage_path
    }
}
