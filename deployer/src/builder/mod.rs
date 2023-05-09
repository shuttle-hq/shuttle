use std::path::PathBuf;

use tokio::{
    fs::File,
    io::{AsyncReadExt, BufReader},
};
use tracing::debug;

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

    /// This method consumes a source_code_archive and returns a deployable Docker image archive.
    pub async fn default_image_archive(&self, source_code_archive: &Vec<u8>) -> Vec<u8> {
        debug!(
            "MockedBuilder received a source code archive of length: {}",
            source_code_archive.len()
        );
        let mut archive =
            BufReader::new(File::open(&self.default_image_archive_path).await.unwrap());
        let mut buf = Vec::new();
        archive.read_to_end(&mut buf).await.unwrap();
        debug!("MockedBuilder returing an image of length: {}", buf.len());
        buf
    }

    /// This method pushes a built image to an archive registry.
    pub async fn push_archive(&self) {
        // TODO
    }
}
