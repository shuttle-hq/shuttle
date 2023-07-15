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
    #[error("failed to read folder contents: {0}")]
    ReadFolder(std::io::Error),
    #[error("failed to read the entry in the folder: {0}")]
    ReadEntry(std::io::Error),
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
            let key = entry.map_err(PersistError::ListFolder)?;
            let key_name = key
                .path()
                .file_stem()
                .unwrap_or_default()
                .to_str()
                .unwrap_or("file name contains non-UTF-8 characters")
                .to_string();
            list.push(key_name);
        }
        Ok(list)
    }

    /// clear method removes the storage folder from the PersistInstance
    pub fn clear(&self) -> Result<(), PersistError> {
        let storage_folder = self.get_storage_folder();
        let entries = fs::read_dir(storage_folder).map_err(PersistError::ReadFolder)?;

        for entry in entries {
            let entry = entry.map_err(PersistError::ReadEntry)?;
            let path = entry.path();

            if path.is_file() {
                fs::remove_file(path).map_err(PersistError::RemoveFile)?;
            }
        }
        Ok(())
    }

    /// remove method deletes a key from the PersistInstance
    pub fn remove(&self, key: &str) -> Result<(), PersistError> {
        let file_path = self.get_storage_file(key);
        fs::remove_file(file_path).map_err(PersistError::RemoveFile)?;
        Ok(())
    }

    /// size method determines the size of all keys stored within a folder, within the PersistInstance
    pub fn size(&self) -> Result<u64, PersistError> {
        let storage_folder = self.get_storage_folder();
        let mut size = 0;

        let entries = fs::read_dir(storage_folder).map_err(PersistError::ReadFolder)?;

        for entry in entries {
            let entry = entry.map_err(PersistError::ReadEntry)?;
            let path = entry.path();

            if path.is_file() {
                size += fs::metadata(&path).map_err(PersistError::ReadEntry)?.len();
            }
        }
        Ok(size)
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
    use rand::Rng;
    use std::str::FromStr;

    fn get_range() -> usize {
        let mut rng = rand::thread_rng();
        let num_keys = rng.gen_range(1..=20);
        num_keys
    }

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
        let list_length = get_range();
        for list_key in 1..=list_length {
            let key_name = format!("list_key_{}", list_key);
            let key_value = format!("list_key_value_{}", list_key);
            persist.save(&key_name, &key_value).unwrap();
        }

        let length = persist.list().unwrap().len();
        assert_eq!(length, list_length);
        persist.clear().unwrap();
    }

    #[test]
    fn test_clear() {
        let persist = PersistInstance {
            service_name: ServiceName::from_str("test_clear").unwrap(),
        };

        let list_length = get_range();
        for clear_key in 1..=list_length {
            let key_name = format!("clear_key_{}", clear_key);
            let key_value = format!("clear_key_value_{}", clear_key);
            persist.save(&key_name, &key_value).unwrap();
        }
        persist.clear().unwrap();
        let actual_length = persist.list().unwrap().len();
        assert_eq!(actual_length, 0);
    }

    #[test]
    fn test_remove() {
        let persist = PersistInstance {
            service_name: ServiceName::from_str("test_remove").unwrap(),
        };

        let list_length = get_range();
        for remove_key in 1..=list_length {
            let key_name = format!("remove_key_{}", remove_key);
            let key_value = format!("remove_key_value_{}", remove_key);
            persist.save(&key_name, &key_value).unwrap();
        }
        persist
            .remove(persist.list().unwrap()[list_length - 1].as_str())
            .unwrap();
        let actual_length = persist.list().unwrap().len();
        assert_eq!(actual_length, list_length - 1);
        persist.clear().unwrap();
    }

    #[test]
    fn test_size() {
        let persist = PersistInstance {
            service_name: ServiceName::from_str("test_size").unwrap(),
        };

        let mut expected_size = 0;
        let list_length = get_range();
        for size_key in 1..=list_length {
            let key_name = format!("size_key_{}", size_key);
            let key_value = format!("size_key_value_{}", size_key);
            persist.save(&key_name, &key_value).unwrap();
            expected_size += fs::metadata(persist.get_storage_file(&key_name))
                .unwrap()
                .len();
        }
        let actual_size = persist.size().unwrap();
        assert_eq!(expected_size, actual_size);
        persist.clear().unwrap();
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
