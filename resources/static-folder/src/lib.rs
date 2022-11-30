use async_trait::async_trait;
use shuttle_service::{Factory, ResourceBuilder};
use std::{fs::rename, path::PathBuf};
use tokio::runtime::Runtime;

pub struct StaticFolder;

#[async_trait]
impl ResourceBuilder<PathBuf> for StaticFolder {
    fn new() -> Self {
        Self {}
    }

    async fn build(
        self,
        factory: &mut dyn Factory,
        _runtime: &Runtime,
    ) -> Result<PathBuf, shuttle_service::Error> {
        let input_dir = factory.get_build_path().join("static");
        let output_dir = factory.get_storage_path().join("static");

        rename(input_dir, output_dir.clone())?;

        Ok(output_dir)
    }
}

#[cfg(test)]
mod tests {
    use std::fs::{self};

    use async_trait::async_trait;
    use shuttle_service::{Factory, ResourceBuilder};
    use tempdir::TempDir;

    use crate::StaticFolder;

    struct MockFactory {
        build_path: TempDir,
        storage_path: TempDir,
    }

    impl MockFactory {
        fn new() -> Self {
            Self {
                build_path: TempDir::new("build").unwrap(),
                storage_path: TempDir::new("storage").unwrap(),
            }
        }
    }

    #[async_trait]
    impl Factory for MockFactory {
        async fn get_db_connection_string(
            &mut self,
            _db_type: shuttle_service::database::Type,
        ) -> Result<String, shuttle_service::Error> {
            panic!("no static folder test should try to get a db connection string")
        }

        async fn get_secrets(
            &mut self,
        ) -> Result<std::collections::BTreeMap<String, String>, shuttle_service::Error> {
            panic!("no static folder test should try to get secrets")
        }

        fn get_service_name(&self) -> shuttle_service::ServiceName {
            panic!("no static folder test should try to get the service name")
        }

        fn get_build_path(&self) -> std::path::PathBuf {
            self.build_path.path().to_owned()
        }

        fn get_storage_path(&self) -> std::path::PathBuf {
            self.storage_path.path().to_owned()
        }
    }

    #[tokio::test]
    async fn copies_folder() {
        let mut factory = MockFactory::new();

        let input_file_path = factory.build_path.path().join("static").join("note.txt");
        fs::create_dir_all(input_file_path.parent().unwrap()).unwrap();
        fs::write(input_file_path, "Hello, test!").unwrap();

        let expected_file = factory.storage_path.path().join("static").join("note.txt");
        assert!(!expected_file.exists(), "input file should not exist yet");

        // Call plugin
        let static_folder = StaticFolder;

        let runtime = tokio::runtime::Runtime::new().unwrap();
        let actual_folder = static_folder.build(&mut factory, &runtime).await.unwrap();

        assert_eq!(
            actual_folder,
            factory.storage_path.path().join("static"),
            "expect path to the static folder"
        );
        assert!(expected_file.exists(), "expected input file to be created");
        assert_eq!(
            fs::read_to_string(expected_file).unwrap(),
            "Hello, test!",
            "expected file content to match"
        );

        runtime.shutdown_background();
    }
}
