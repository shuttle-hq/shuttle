use async_trait::async_trait;
use bincode::{deserialize_from, serialize_into, Error as BincodeError};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use shuttle_common::project::ProjectName;
use shuttle_service::Type;
use shuttle_service::{Factory, ResourceBuilder};
use std::fs;
use std::fs::File;
use std::io::BufReader;
use std::io::BufWriter;
use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum PersistError {
    #[error("failed to open file: {0}")]
    Open(std::io::Error),
    #[error("failed to create folder: {0}")]
    CreateFolder(std::io::Error),
    #[error("failed to serialize data: {0}")]
    Serialize(BincodeError),
    #[error("failed to deserialize data: {0}")]
    Deserialize(BincodeError),
}

#[derive(Serialize)]
pub struct Persist;

#[derive(Deserialize, Serialize, Clone)]
pub struct PersistInstance {
    service_name: ProjectName,
}

impl PersistInstance {
    pub fn save<T: Serialize>(&self, key: &str, struc: T) -> Result<(), PersistError> {
        let storage_folder = self.get_storage_folder();
        fs::create_dir_all(storage_folder).map_err(PersistError::CreateFolder)?;

        let file_path = self.get_storage_file(key);
        let file = File::create(file_path).map_err(PersistError::Open)?;
        let mut writer = BufWriter::new(file);
        Ok(serialize_into(&mut writer, &struc).map_err(PersistError::Serialize))?
    }

    pub fn load<T>(&self, key: &str) -> Result<T, PersistError>
    where
        T: DeserializeOwned,
    {
        let file_path = self.get_storage_file(key);
        let file = File::open(file_path).map_err(PersistError::Open)?;
        let reader = BufReader::new(file);
        Ok(deserialize_from(reader).map_err(PersistError::Deserialize))?
    }

    fn get_storage_folder(&self) -> PathBuf {
        ["shuttle_persist", &self.service_name.to_string()]
            .iter()
            .collect()
    }

    fn get_storage_file(&self, key: &str) -> PathBuf {
        let mut path = self.get_storage_folder();
        path.push(format!("{key}.bin"));

        path
    }
}

#[async_trait]
impl ResourceBuilder<PersistInstance> for Persist {
    const TYPE: Type = Type::Persist;

    type Config = ();

    type Output = PersistInstance;

    fn new() -> Self {
        Self {}
    }

    fn config(&self) -> &Self::Config {
        &()
    }

    async fn output(
        self,
        factory: &mut dyn Factory,
    ) -> Result<Self::Output, shuttle_service::Error> {
        Ok(PersistInstance {
            service_name: factory.get_service_name(),
        })
    }

    async fn build(build_data: &Self::Output) -> Result<PersistInstance, shuttle_service::Error> {
        Ok(build_data.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shuttle_common::project::ProjectName;
    use std::str::FromStr;

    #[test]
    fn test_save_and_load() {
        let persist = PersistInstance {
            service_name: ProjectName::from_str("test").unwrap(),
        };

        persist.save("test", "test").unwrap();
        let result: String = persist.load("test").unwrap();
        assert_eq!(result, "test");
    }

    #[test]
    fn test_load_error() {
        let persist = PersistInstance {
            service_name: ProjectName::from_str("test").unwrap(),
        };

        // unwrapp error
        let result = persist.load::<String>("error").unwrap_err();
        assert_eq!(
            result.to_string(),
            "failed to open file: No such file or directory (os error 2)"
        );
    }
}
