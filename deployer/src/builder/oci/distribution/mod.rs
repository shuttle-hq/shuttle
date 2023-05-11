//! Pull and Push images to OCI registry based on [OCI distribution specification](https://github.com/opencontainers/distribution-spec)

mod auth;
mod client;
mod name;
mod reference;

pub use auth::*;
pub use client::Client;
pub use name::Name;
pub use reference::Reference;
use tracing::info;

use super::{digest::Digest, error::*};
use std::{fs, io::Read, path::Path};

/// Push image to registry
pub fn push_image(path: &Path) -> Result<()> {
    if !path.is_file() {
        return Err(Error::NotAFile(path.to_owned()));
    }
    let mut f = fs::File::open(path)?;
    let mut ar = super::image::Archive::new(&mut f);
    for (image_name, manifest) in ar.get_manifests()? {
        info!("Push image: {}", image_name);
        let mut client = Client::new(image_name.registry_url()?, image_name.name)?;
        for layer in manifest.layers() {
            let digest = Digest::new(layer.digest())?;
            let mut entry = ar.get_blob(&digest)?;
            let mut buf = Vec::new();
            entry.read_to_end(&mut buf)?;
            client.push_blob(&buf)?;
        }
        let digest = Digest::new(manifest.config().digest())?;
        let mut entry = ar.get_blob(&digest)?;
        let mut buf = Vec::new();
        entry.read_to_end(&mut buf)?;
        client.push_blob(&buf)?;
        client.push_manifest(&image_name.reference, &manifest)?;
    }
    Ok(())
}
