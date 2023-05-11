use std::path::{Path, PathBuf};

use tracing::debug;

mod oci;

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
    pub async fn build_and_push_image(&self, source_code_archive: &Vec<u8>) -> uuid::Uuid {
        debug!(
            "MockedBuilder received a source code archive of length: {}. Now building it...",
            source_code_archive.len()
        );
        self.push_image(&self.default_image_archive_path).await;
        debug!("Successfuly built and pushed the image to the container registry.");

        uuid::Uuid::new_v4()
    }

    /// Push a built image to an container registry.
    pub async fn push_image(&self, image_path: &Path) {
        self::oci::distribution::push_image(image_path).expect("to not fail");
    }
}
