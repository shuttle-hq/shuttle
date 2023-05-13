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

use super::{error::*, image::digest::Digest};

/// Push image to registry
pub async fn push_image(image: Vec<u8>) -> Result<()> {
    let mut ar = super::image::Archive::new(&image);
    let manifests = ar.get_manifests().await?;

    for (image_name, manifest) in manifests {
        info!(%image_name, "pushing image");
        let mut client = Client::new(image_name.registry_url()?, image_name.name)?;
        for layer in manifest.layers() {
            let digest = Digest::new(layer.digest())?;
            client.push_blob(ar.get_blob(&digest).await?).await?;
        }
        let digest = Digest::new(manifest.config().digest())?;
        client.push_blob(ar.get_blob(&digest).await?).await?;
        client
            .push_manifest(&image_name.reference, &manifest)
            .await?;
    }
    Ok(())
}
