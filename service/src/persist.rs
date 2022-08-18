use tokio::runtime::Runtime;

use crate::{error::CustomError, Factory, ResourceBuilder};
use async_trait::async_trait;
use serde::de::DeserializeOwned;
use serde::Serialize;

use std::fs;
use std::fs::File;
use std::io::BufReader;
use std::io::BufWriter;

use bincode::{deserialize_from, serialize_into};

use shuttle_common::project::ProjectName;

pub struct Persist;

pub struct PersistInstance {
    project_name: ProjectName,
}

impl PersistInstance {
    pub fn save<T: Serialize>(&self, key: &str, struc: T) -> Result<(), crate::Error> {
        let project_name = self.project_name.to_string();
        fs::create_dir_all(format!("shuttle_persist/{}", project_name))?;

        let file = File::create(format!("shuttle_persist/{}/{}.bin", project_name, key))?;
        let mut writer = BufWriter::new(file);
        Ok(serialize_into(&mut writer, &struc).map_err(CustomError::new)?)
    }

    pub fn load<T>(&self, key: &str) -> Result<T, crate::Error>
    where
        T: DeserializeOwned,
    {
        let project_name = self.project_name.to_string();
        let file = File::open(format!("shuttle_persist/{}/{}.bin", project_name, key))?;
        let reader = BufReader::new(file);
        Ok(deserialize_from(reader).map_err(CustomError::new)?)
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
    fn test_create_folder() {
        let path = format!("shuttle_persist/{}", "test".to_string());
        assert!(fs::metadata(path).is_ok());
    }

    #[test]
    fn test_save_and_load() {
        let persist = PersistInstance {
            project_name: ProjectName::from_str("test").unwrap(),
        };

        persist.save("test", "test").unwrap();
        let result: String = persist.load("test").unwrap();
        assert_eq!(result, "test");
    }
}
