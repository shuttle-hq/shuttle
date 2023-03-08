use async_trait::async_trait;
use fs_extra::dir::{copy, CopyOptions};
use shuttle_service::{
    error::{CustomError, Error as ShuttleError},
    Factory, ResourceBuilder,
};
use std::path::{Path, PathBuf};
use tokio::runtime::Runtime;
use tracing::{error, trace};

pub struct StaticFolder<'a> {
    /// The folder to reach at runtime. Defaults to `static`
    folder: &'a str,
}

pub enum Error {
    AbsolutePath,
    TransversedUp,
    Copy(fs_extra::error::Error),
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

        trace!(?folder, "building static folder");

        // Prevent users from users from reading anything outside of their crate's build folder
        if folder.is_absolute() {
            error!("the static folder cannot be an absolute path");
            return Err(Error::AbsolutePath)?;
        }

        let input_dir = factory.get_build_path()?.join(self.folder);

        trace!(input_directory = ?input_dir, "got input directory");

        match input_dir.canonicalize() {
            Ok(canonical_path) if canonical_path != input_dir => return Err(Error::TransversedUp)?,
            Ok(_) => {
                // The path did not change to outside the crate's build folder
            }
            Err(err) => {
                error!(
                    error = &err as &dyn std::error::Error,
                    "failed to get static folder"
                );
                return Err(err)?;
            }
        }

        let output_dir = factory.get_storage_path()?;

        trace!(output_directory = ?output_dir, "got output directory");

        let copy_options = CopyOptions::new().overwrite(true);
        match copy(&input_dir, &output_dir, &copy_options) {
            Ok(_) => Ok(output_dir.join(self.folder)),
            Err(error) => {
                error!(
                    error = &error as &dyn std::error::Error,
                    "failed to copy static folder"
                );

                Err(Error::Copy(error))?
            }
        }
    }
}

impl From<Error> for shuttle_service::Error {
    fn from(error: Error) -> Self {
        let msg = match error {
            Error::AbsolutePath => "Cannot use an absolute path for a static folder".to_string(),
            Error::TransversedUp => {
                "Cannot transverse out of crate for a static folder".to_string()
            }
            Error::Copy(error) => format!("Cannot copy static folder: {}", error),
        };

        ShuttleError::Custom(CustomError::msg(msg))
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use async_trait::async_trait;
    use shuttle_service::{Factory, ResourceBuilder};
    use tempfile::{Builder, TempDir};

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
                temp_dir: Builder::new().prefix("static_folder").tempdir().unwrap(),
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

        fn get_environment(&self) -> shuttle_service::Environment {
            panic!("no static folder test should try to get the environment")
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
