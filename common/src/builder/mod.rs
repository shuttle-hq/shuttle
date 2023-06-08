use std::path::PathBuf;

use tokio::{fs::File, io::AsyncReadExt};
use tracing::debug;

use crate::builder::error::Error;

use self::oci::error::Result;

pub mod error;
pub mod oci;

#[derive(Clone)]
pub struct MockedBuilder {
    default_image_archive_path: PathBuf,
}

impl MockedBuilder {
    /// Instantiate a new MockedBuilder.
    pub fn new(image_archive_path: PathBuf) -> Self {
        MockedBuilder {
            default_image_archive_path: image_archive_path,
        }
    }

    /// Consume a `source_code_archive` and return a deployment_id.
    pub async fn build_and_push_image(
        &self,
        source_code_archive: &Vec<u8>,
    ) -> error::Result<uuid::Uuid> {
        debug!(
            "MockedBuilder received a source code archive of length: {}. Now building it...",
            source_code_archive.len()
        );

        if !self.default_image_archive_path.is_file() {
            return Err(crate::builder::error::Error::Oci(
                crate::builder::oci::error::Error::NotAFile(
                    self.default_image_archive_path.clone(),
                ),
            ));
        }
        let mut f = File::open(self.default_image_archive_path.as_path())
            .await
            .expect("to open the file");
        let mut buf = Vec::new();
        f.read_to_end(&mut buf).await?;

        // We do not expect multi-arch images to be returned from the builder, so we're expecting
        // a single image name, corresponding to a single image manifest, for a single architecture.
        let image_names = self.push_image(buf).await.map_err(Error::Oci)?;
        if image_names.len() == 1 {
            todo!();
        }

        Ok(uuid::Uuid::new_v4())
    }

    /// Push an image (including multi-arch manifests) to a container registry
    /// and get the associated image name.
    pub async fn push_image(&self, image: Vec<u8>) -> Result<Vec<String>> {
        self::oci::distribution::push_image(image).await
    }
}
