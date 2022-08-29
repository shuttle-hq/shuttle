use crate::{Factory, ResourceBuilder};
use async_trait::async_trait;
use bincode::{deserialize_from, serialize_into, Error as BincodeError};
use serde::de::DeserializeOwned;
use serde::Serialize;
use shuttle_common::project::ProjectName;
use std::fs;
use std::fs::File;
use std::io::BufReader;
use std::io::BufWriter;
use thiserror::Error;
use tokio::runtime::Runtime;

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

pub struct Persist;

pub struct PersistInstance {
    project_name: ProjectName,
}

impl PersistInstance {
    pub fn save<T: Serialize>(&self, key: &str, struc: T) -> Result<(), PersistError> {
        let project_name = self.project_name.to_string();
        fs::create_dir_all(format!("shuttle_persist/{}", project_name))
            .map_err(PersistError::CreateFolder)?;

        let file = File::create(format!("shuttle_persist/{}/{}.bin", project_name, key))
            .map_err(PersistError::Open)?;
        let mut writer = BufWriter::new(file);
        Ok(serialize_into(&mut writer, &struc).map_err(PersistError::Serialize))?
    }

    pub fn load<T>(&self, key: &str) -> Result<T, PersistError>
    where
        T: DeserializeOwned,
    {
        let project_name = self.project_name.to_string();
        let file = File::open(format!("shuttle_persist/{}/{}.bin", project_name, key))
            .map_err(PersistError::Open)?;
        let reader = BufReader::new(file);
        Ok(deserialize_from(reader).map_err(PersistError::Deserialize))?
    }
}

#[async_trait]
impl ResourceBuilder<PersistInstance> for Persist {
    fn new() -> Self {
        Self {}
    }

    async fn build(
        self,
        factory: &mut dyn Factory,
        _runtime: &Runtime,
    ) -> Result<PersistInstance, crate::Error> {
        Ok(PersistInstance {
            project_name: factory.get_project_name(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shuttle_common::project::ProjectName;
    use std::{fs, str::FromStr};

    #[test]
    fn test_save_and_load() {
        let persist = PersistInstance {
            project_name: ProjectName::from_str("test").unwrap(),
        };

        persist.save("test", "test").unwrap();
        let result: String = persist.load("test").unwrap();
        assert_eq!(result, "test");
    }

    #[test]
    fn test_load_error() {
        let persist = PersistInstance {
            project_name: ProjectName::from_str("test").unwrap(),
        };

        // unwrapp error
        let result = persist.load::<String>("error").unwrap_err();
        assert_eq!(
            result.to_string(),
            "failed to open file: No such file or directory (os error 2)"
        );
    }
}
