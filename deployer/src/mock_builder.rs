pub struct MockedBuilder {
    default_image_archive_path: Path,
}

impl MockedBuilder {
    /// This method consumes a source_code_archive and returns a deployable Docker image archive.
    pub async fn get_default_image_archive() -> Vec<u8> {
        /// TODO: read from the defualt_image_archive_path.
        Vec::new()
    }
}
