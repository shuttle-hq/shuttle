use async_trait::async_trait;
use shuttle_service::{
    error::{CustomError, Error as ShuttleError},
    Factory, ResourceBuilder,
};
use std::{
    fs::rename,
    path::{Path, PathBuf},
};
use tokio::runtime::Runtime;

pub struct StaticFolder<'a> {
    /// The folder to reach at runtime. Defaults to `static`
    folder: &'a str,
}

pub enum Error {
    AbsolutePath,
    TransversedUp,
}

impl<'a> StaticFolder<'a> {
    pub fn folder(mut self, folder: &'a str) -> Self {
        self.folder = folder;

        self
    }
}

#[async_trait]
impl<'a> ResourceBuilder<PathBuf> for StaticFolder<'a> {
    fn new() -> Self {
        Self { folder: "static" }
    }

    async fn build(
        self,
        factory: &mut dyn Factory,
        _runtime: &Runtime,
    ) -> Result<PathBuf, shuttle_service::Error> {
        let folder = Path::new(self.folder);

        // Prevent users from users from reading anything outside of their crate's build folder
        if folder.is_absolute() {
            return Err(Error::AbsolutePath)?;
        }

        let input_dir = factory.get_build_path()?.join(self.folder);

        match input_dir.canonicalize() {
            Ok(canonical_path) if canonical_path != input_dir => return Err(Error::TransversedUp)?,
            Ok(_) => {
                // The path did not change to outside the crate's build folder
            }
            Err(err) => return Err(err)?,
        }

        let output_dir = factory.get_storage_path()?.join(self.folder);

        rename(input_dir, output_dir.clone())?;

        Ok(output_dir)
    }
}

impl From<Error> for shuttle_service::Error {
    fn from(error: Error) -> Self {
        let msg = match error {
            Error::AbsolutePath => "Cannot use an absolute path for a static folder",
            Error::TransversedUp => "Cannot transverse out of crate for a static folder",
        };

        ShuttleError::Custom(CustomError::msg(msg))
    }
}

#[cfg(test)]
mod tests {
    use std::fs::{self};
    use std::path::PathBuf;

    use async_trait::async_trait;
    use shuttle_service::{Factory, ResourceBuilder};
    use tempdir::TempDir;

    use crate::StaticFolder;

    struct MockFactory {
        temp_dir: TempDir,
    }

    // Will have this tree across all the tests
    // .
    // ├── build
    // │   └── static
    // │       └── note.txt
    // ├── storage
    // │   └── static
    // │       └── note.txt
    // └── escape
    //     └── passwd
    impl MockFactory {
        fn new() -> Self {
            Self {
                temp_dir: TempDir::new("static_folder").unwrap(),
            }
        }

        fn build_path(&self) -> PathBuf {
            self.get_path("build")
        }

        fn storage_path(&self) -> PathBuf {
            self.get_path("storage")
        }

        fn escape_path(&self) -> PathBuf {
            self.get_path("escape")
        }

        fn get_path(&self, folder: &str) -> PathBuf {
            let path = self.temp_dir.path().join(folder);

            if !path.exists() {
                fs::create_dir(&path).unwrap();
            }

            path
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

        fn get_build_path(&self) -> Result<std::path::PathBuf, shuttle_service::Error> {
            Ok(self.build_path())
        }

        fn get_storage_path(&self) -> Result<std::path::PathBuf, shuttle_service::Error> {
            Ok(self.storage_path())
        }
    }

    #[tokio::test]
    async fn copies_folder() {
        let mut factory = MockFactory::new();

        let input_file_path = factory.build_path().join("static").join("note.txt");
        fs::create_dir_all(input_file_path.parent().unwrap()).unwrap();
        fs::write(input_file_path, "Hello, test!").unwrap();

        let expected_file = factory.storage_path().join("static").join("note.txt");
        assert!(!expected_file.exists(), "input file should not exist yet");

        // Call plugin
        let static_folder = StaticFolder::new();

        let runtime = tokio::runtime::Runtime::new().unwrap();
        let actual_folder = static_folder.build(&mut factory, &runtime).await.unwrap();

        assert_eq!(
            actual_folder,
            factory.storage_path().join("static"),
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

    #[tokio::test]
    #[should_panic(expected = "Cannot use an absolute path for a static folder")]
    async fn cannot_use_absolute_path() {
        let mut factory = MockFactory::new();
        let static_folder = StaticFolder::new();
        let runtime = tokio::runtime::Runtime::new().unwrap();

        let _ = static_folder
            .folder("/etc")
            .build(&mut factory, &runtime)
            .await
            .unwrap();

        runtime.shutdown_background();
    }

    #[tokio::test]
    #[should_panic(expected = "Cannot transverse out of crate for a static folder")]
    async fn cannot_transverse_up() {
        let mut factory = MockFactory::new();

        let password_file_path = factory.escape_path().join("passwd");
        fs::create_dir_all(password_file_path.parent().unwrap()).unwrap();
        fs::write(password_file_path, "qwerty").unwrap();

        // Call plugin
        let static_folder = StaticFolder::new();

        let runtime = tokio::runtime::Runtime::new().unwrap();
        let _ = static_folder
            .folder("../escape")
            .build(&mut factory, &runtime)
            .await
            .unwrap();

        runtime.shutdown_background();
    }
}
