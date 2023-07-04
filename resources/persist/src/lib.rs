use async_trait::async_trait;
use bincode::{deserialize_from, serialize_into, Error as BincodeError};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use shuttle_service::{Factory, ResourceBuilder, ServiceName, Type};
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
    #[error("failed to list contents of folder: {0}")]
    ListFolder(std::io::Error),
    #[error("failed to clear the folder: {0}")]
    ClearFolder(std::io::Error),
    #[error("failed to remove file: {0}")]
    RemoveFile(std::io::Error),
    #[error("failed to serialize data: {0}")]
    Serialize(BincodeError),
    #[error("failed to deserialize data: {0}")]
    Deserialize(BincodeError),
}

#[derive(Serialize)]
pub struct Persist;

#[derive(Deserialize, Serialize, Clone)]
pub struct PersistInstance {
    service_name: ServiceName,
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

    /// list method returns a vector of strings containing all the keys associated with a PersistInstance
    pub fn list(&self) -> Result<Vec<String>, PersistError> {
        let storage_folder = self.get_storage_folder();

        let mut list = Vec::new();

        let entries = fs::read_dir(storage_folder).map_err(PersistError::ListFolder)?;
        for entry in entries {
            let dir = entry.map_err(PersistError::ListFolder)?;
            list.push(
                dir.path()
                    .into_os_string()
                    .into_string()
                    .unwrap_or_else(|_| "path contains non-UTF-8 characters".to_string()),
            );
        }
        Ok(list)
    }

    /// clear method removes the storage folder from the PersistInstance
    pub fn clear(&self) -> Result<(), PersistError> {
        let folder_path = self.get_storage_folder();
        fs::remove_dir_all(folder_path).map_err(PersistError::ClearFolder)?;
        Ok(())
    }

    /// remove method deletes a key from the PersistInstance
    pub fn remove(&self, list_item: String) -> Result<(), PersistError> {
        let file_path = self.get_storage_file(&list_item);
        fs::remove_file(file_path).map_err(PersistError::RemoveFile)?;
        Ok(())
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
    use std::str::FromStr;

    #[test]
    fn test_save_and_load() {
        let persist = PersistInstance {
            service_name: ServiceName::from_str("test").unwrap(),
        };

        persist.save("test", "test").unwrap();
        let result: String = persist.load("test").unwrap();
        assert_eq!(result, "test");
    }

    #[test]
    fn test_list() {
        let persist = PersistInstance {
            service_name: ServiceName::from_str("test_list").unwrap(),
        };

        persist.save("test_list", "test_list").unwrap();

        let result = vec!["shuttle_persist/test_list/test_list.bin".to_string()];
        let list_result = persist.list().unwrap();
        assert_eq!(result, list_result);
    }

    #[test]
    fn test_clear() {
        let persist = PersistInstance {
            service_name: ServiceName::from_str("test_clear").unwrap(),
        };

        persist.save("test_clear", "test_clear").unwrap();
        persist.clear().unwrap();
        let clear_result = persist.list().unwrap_err();
        assert_eq!(
            clear_result.to_string(),
            "failed to list contents of folder: No such file or directory (os error 2)"
        );
    }

    #[test]
    fn test_remove() {
        let persist = PersistInstance {
            service_name: ServiceName::from_str("test_remove").unwrap(),
        };

        persist.save("test_remove", "test_remove").unwrap();
        let item_to_remove = "test_remove".to_string();
        persist.remove(item_to_remove).unwrap();
        assert!(persist.list().unwrap().is_empty());
    }

    #[test]
    fn test_load_error() {
        let persist = PersistInstance {
            service_name: ServiceName::from_str("test").unwrap(),
        };

        // unwrap error
        let result = persist.load::<String>("error").unwrap_err();
        assert_eq!(
            result.to_string(),
            "failed to open file: No such file or directory (os error 2)"
        );
    }
}
