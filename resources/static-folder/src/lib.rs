use std::path::PathBuf;

use async_trait::async_trait;
use serde::Serialize;
use shuttle_service::{Error, Factory, ResourceBuilder, Type};

#[derive(Serialize)]
#[deprecated(
    since = "0.28.0",
    note = "Folder names can now be hard coded. More about deployment files: https://docs.shuttle.rs/configuration/files"
)]
pub struct StaticFolder<'a> {
    /// The folder to reach at runtime. Defaults to `static`
    folder: &'a str,
}

impl<'a> StaticFolder<'a> {
    pub fn folder(mut self, folder: &'a str) -> Self {
        self.folder = folder;

        self
    }
}

#[async_trait]
impl<'a> ResourceBuilder<PathBuf> for StaticFolder<'a> {
    const TYPE: Type = Type::StaticFolder;

    type Config = &'a str;

    type Output = PathBuf;

    fn new() -> Self {
        Self { folder: "static" }
    }

    fn config(&self) -> &&'a str {
        &self.folder
    }

    async fn output(self, _factory: &mut dyn Factory) -> Result<Self::Output, Error> {
        Ok(PathBuf::from(self.folder))
    }

    async fn build(build_data: &Self::Output) -> Result<PathBuf, Error> {
        Ok(build_data.clone())
    }
}
