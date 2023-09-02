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
    #[error("failed to list file name: {0}")]
    ListName(String),
    #[error("failed to clear folder: {0}")]
    RemoveFolder(std::io::Error),
    #[error("failed to remove file: {0}")]
    RemoveFile(std::io::Error),
    #[error("failed to serialize data: {0}")]
    Serialize(BincodeError),
    #[error("failed to deserialize data: {0}")]
    Deserialize(BincodeError),
}

#[derive(Serialize)]
pub struct Persist;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PersistInstance {
    service_name: ServiceName,
}

impl PersistInstance {
    /// Constructs a new PersistInstance along with its associated storage folder
    pub fn new(service_name: ServiceName) -> Result<Self, PersistError> {
        let instance = Self { service_name };
        let storage_folder = instance.get_storage_folder();
        fs::create_dir_all(storage_folder).map_err(PersistError::CreateFolder)?;

        Ok(instance)
    }

    pub fn save<T: Serialize>(&self, key: &str, struc: T) -> Result<(), PersistError> {
        let file_path = self.get_storage_file(key);
        let file = File::create(file_path).map_err(PersistError::Open)?;
        let mut writer = BufWriter::new(file);

        Ok(serialize_into(&mut writer, &struc).map_err(PersistError::Serialize))?
    }

    /// Returns a vector of strings containing all the keys associated with a PersistInstance
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
                .ok_or(PersistError::ListName(
                    "the file name contains invalid characters".to_owned(),
                ))?
                .to_string();
            list.push(key_name);
        }

        Ok(list)
    }

    /// Removes the keys within the PersistInstance
    pub fn clear(&self) -> Result<(), PersistError> {
        let storage_folder = self.get_storage_folder();
        fs::remove_dir_all(&storage_folder).map_err(PersistError::RemoveFolder)?;
        fs::create_dir_all(&storage_folder).map_err(PersistError::CreateFolder)?;

        Ok(())
    }

    /// Returns the number of keys in a folder within a PersistInstance
    pub fn size(&self) -> Result<usize, PersistError> {
        Ok(self.list()?.len())
    }

    /// Deletes a key from the PersistInstance
    pub fn remove(&self, key: &str) -> Result<(), PersistError> {
        let file_path = self.get_storage_file(key);
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
        let persist_instance = match PersistInstance::new(factory.get_service_name()) {
            Ok(persist_instance) => persist_instance,
            Err(e) => return Err(shuttle_service::Error::Custom(e.into())),
        };

        Ok(persist_instance)
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
        let persist = PersistInstance::new(ServiceName::from_str("test").unwrap()).unwrap();

        persist.save("test", "test").unwrap();
        let result: String = persist.load("test").unwrap();
        assert_eq!(result, "test");
    }

    #[test]
    fn test_list_and_size() {
        let persist = PersistInstance::new(ServiceName::from_str("test1").unwrap()).unwrap();

        persist.save("test", "test").unwrap();
        let list_result = persist.list().unwrap().len();
        let size_result = persist.size().unwrap();
        assert_eq!(list_result, 1);
        assert_eq!(size_result, 1);
    }

    #[test]
    fn test_remove() {
        let persist = PersistInstance::new(ServiceName::from_str("test3").unwrap()).unwrap();

        persist.save("test", "test").unwrap();
        persist.save("test2", "test2").unwrap();
        let list = persist.list().unwrap();
        let key = list[0].as_str();
        persist.remove(key).unwrap();
        let result = persist.list().unwrap();
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_remove_error() {
        let persist = PersistInstance::new(ServiceName::from_str("test4").unwrap()).unwrap();

        // unwrap error
        let result = persist.remove("test4").unwrap_err();
        assert_eq!(
            result.to_string(),
            "failed to remove file: No such file or directory (os error 2)"
        );
    }

    #[test]
    fn test_clear() {
        let persist = PersistInstance::new(ServiceName::from_str("test5").unwrap()).unwrap();

        persist.save("test5", "test5").unwrap();
        persist.clear().unwrap();
        let result = persist.list().unwrap();
        assert_eq!(result.len(), 0);
    }

    #[test]
    fn test_load_error() {
        let persist = PersistInstance::new(ServiceName::from_str("test").unwrap()).unwrap();

        // unwrap error
        let result = persist.load::<String>("error").unwrap_err();
        assert_eq!(
            result.to_string(),
            "failed to open file: No such file or directory (os error 2)"
        );
    }
}
