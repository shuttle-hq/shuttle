use super::error::Error;
use super::{distribution::Name, distribution::Reference, error::Result};
use digest::Digest;
use oci_spec::image::{ImageIndex, ImageManifest};
use std::fmt;
use std::io::{Read, Seek};

use url::Url;

mod annotations;
pub mod digest;

/// Image name
///
/// The input must be valid both as "org.opencontainers.image.ref.name"
/// defined in pre-defined annotation keys in [OCI image spec]:
///
/// ```text
/// ref       ::= component ("/" component)*
/// component ::= alphanum (separator alphanum)*
/// alphanum  ::= [A-Za-z0-9]+
/// separator ::= [-._:@+] | "--"
/// ```
///
/// and as an argument for [docker tag].
///
/// [OCI image spec]: https://github.com/opencontainers/image-spec/blob/main/annotations.md#pre-defined-annotation-keys
/// [docker tag]: https://docs.docker.com/engine/reference/commandline/tag/
///
/// Terminology
/// ------------
/// We call each components of image name to match OCI distribution spec:
///
/// ```text
/// ghcr.io/termoshtt/ocipkg/testing:latest
/// ^^^^^^^---------------------------------- hostname
///         ^^^^^^^^^^^^^^^^^^^^^^^^--------- name
///                                  ^^^^^^-- reference
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ImageName {
    pub hostname: String,
    pub port: Option<u16>,
    pub name: Name,
    pub reference: Reference,
}

impl fmt::Display for ImageName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(port) = self.port {
            write!(
                f,
                "{}:{}/{}:{}",
                self.hostname, port, self.name, self.reference
            )
        } else {
            write!(f, "{}/{}:{}", self.hostname, self.name, self.reference)
        }
    }
}

impl ImageName {
    pub fn parse(name: &str) -> Result<Self> {
        let (hostname, name) = name
            .split_once('/')
            .ok_or(super::error::Error::InvalidName(
                "Couldn't get the hostname of the image".to_string(),
            ))?;
        let (hostname, port) = if let Some((hostname, port)) = hostname.split_once(':') {
            (hostname, Some(str::parse(port)?))
        } else {
            (hostname, None)
        };
        let (name, reference) = name.split_once(':').unwrap_or((name, "latest"));
        Ok(ImageName {
            hostname: hostname.to_string(),
            port,
            name: Name::new(name)?,
            reference: Reference::new(reference)?,
        })
    }

    /// URL for OCI distribution API endpoint
    pub fn registry_url(&self) -> Result<Url> {
        let hostname = if let Some(port) = self.port {
            format!("{}:{}", self.hostname, port)
        } else {
            self.hostname.clone()
        };
        let url = if self.hostname.starts_with("localhost") {
            format!("http://{}", hostname)
        } else {
            format!("https://{}", hostname)
        };
        Ok(Url::parse(&url)?)
    }
}

/// Handler for oci-archive format
///
/// oci-archive consists of several manifests i.e. several container.
pub struct Archive<'buf, W: Read + Seek> {
    archive: Option<tar::Archive<&'buf mut W>>,
}

impl<'buf, W: Read + Seek> Archive<'buf, W> {
    pub fn new(buf: &'buf mut W) -> Self {
        Self {
            archive: Some(tar::Archive::new(buf)),
        }
    }

    pub fn entries(&mut self) -> Result<tar::Entries<&'buf mut W>> {
        let raw = self
            .archive
            .take()
            .expect("This never becomes None except in this function");
        let inner = raw.into_inner();
        inner.rewind()?;
        self.archive = Some(tar::Archive::new(inner));
        Ok(self
            .archive
            .as_mut()
            .expect("This never becomes None except in this function")
            .entries_with_seek()?)
    }

    pub fn get_manifests(&mut self) -> Result<Vec<(ImageName, ImageManifest)>> {
        let index = self.get_index()?;
        index
            .manifests()
            .iter()
            .map(|manifest| {
                let annotations = annotations::Annotations::from_map(
                    manifest.annotations().clone().unwrap_or_default(),
                )?;
                let name = annotations
                    .containerd_image_name
                    .ok_or(Error::MissingManifestName)?;

                let image_name = ImageName::parse(name.as_str())?;
                let digest = Digest::new(manifest.digest())?;
                let manifest = self.get_manifest(&digest)?;
                Ok((image_name, manifest))
            })
            .collect()
    }

    pub fn get_index(&mut self) -> Result<ImageIndex> {
        for entry in self.entries()? {
            let mut entry = entry?;
            if entry.path()?.as_os_str() == "index.json" {
                let mut out = Vec::new();
                entry.read_to_end(&mut out)?;
                return Ok(ImageIndex::from_reader(&*out)?);
            }
        }
        Err(Error::MissingIndex)
    }

    pub fn get_blob(&mut self, digest: &Digest) -> Result<tar::Entry<&'buf mut W>> {
        for entry in self.entries()? {
            let entry = entry?;
            if entry.path()? == digest.as_path() {
                return Ok(entry);
            }
        }
        Err(Error::UnknownDigest(digest.clone()))
    }

    pub fn get_manifest(&mut self, digest: &Digest) -> Result<ImageManifest> {
        let entry = self.get_blob(digest)?;
        Ok(ImageManifest::from_reader(entry)?)
    }
}

#[cfg(test)]
mod tests {
    use crate::builder::oci::distribution::{Name, Reference};

    use super::ImageName;

    const NAME: &str = "shuttle-service";
    const REFERENCE: &str = "latest";

    #[test]
    fn image_to_string() {
        let mut image_name = ImageName {
            hostname: "localhost".to_string(),
            port: None,
            name: Name(NAME.to_string()),
            reference: Reference(REFERENCE.to_string()),
        };
        assert_eq!(
            image_name.to_string(),
            format!("localhost/{}:{}", NAME, REFERENCE)
        );

        image_name.port = Some(5000);
        assert_eq!(
            image_name.to_string(),
            format!(
                "localhost:{}/shuttle-service:latest",
                image_name.port.unwrap()
            )
        );
    }

    #[test]
    fn image_name_parse() {
        assert!(ImageName::parse("localhost").is_err());
        assert!(ImageName::parse("localhost/").is_err());
        assert!(ImageName::parse("localhost:5000/$:0.0.1").is_err());
        assert!(ImageName::parse("localhost:5000/name:$").is_err());

        let image_name = ImageName::parse("localhost/shuttle-service").unwrap();
        assert_eq!(image_name.hostname, "localhost");
        assert_eq!(image_name.port, None);
        assert_eq!(image_name.name.to_string(), "shuttle-service");
        assert_eq!(image_name.reference.to_string(), "latest");

        let image_name = ImageName::parse("localhost/shuttle-service:0.0.1").unwrap();
        assert_eq!(image_name.hostname, "localhost");
        assert_eq!(image_name.port, None);
        assert_eq!(image_name.name.to_string(), "shuttle-service");
        assert_eq!(image_name.reference.to_string(), "0.0.1");

        let image_name = ImageName::parse("localhost:5000/shuttle-service:0.0.1").unwrap();
        assert_eq!(image_name.hostname, "localhost");
        assert_eq!(image_name.port, Some(5000));
        assert_eq!(image_name.name.to_string(), "shuttle-service");
        assert_eq!(image_name.reference.to_string(), "0.0.1");
    }
}
