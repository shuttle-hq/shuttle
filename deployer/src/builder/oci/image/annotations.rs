//! Annotations with flat serialization/deserialization

use super::super::error::*;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, iter::*};

/// `org.opencontainers.image.*` annotations
///
/// See [Pre-Defined Annotation Keys](https://github.com/opencontainers/image-spec/blob/main/annotations.md#pre-defined-annotation-keys)
/// in OCI image spec.
///
/// This is designed to use with JSON
///
/// ```
/// use ocipkg::image::annotations::flat::*;
///
/// let a = Annotations::from_json(r#"
/// {
///   "org.opencontainers.image.url": "https://github.com/termoshtt/ocipkg"
/// }
/// "#).unwrap();
/// assert_eq!(
///     a,
///     Annotations {
///         url: Some("https://github.com/termoshtt/ocipkg".to_string()),
///         ..Default::default()
///     }
/// );
///
/// // Dump to JSON
/// let a = Annotations {
///     url: Some("https://github.com/termoshtt/ocipkg".to_string()),
///     ..Default::default()
/// };
/// assert_eq!(a.to_json().trim(), r#"
/// {
///   "org.opencontainers.image.url": "https://github.com/termoshtt/ocipkg"
/// }
/// "#.trim());
///
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct Annotations {
    /// `org.opencontainers.image.created`
    ///
    /// date and time on which the image was built (string, date-time as defined by RFC 3339).
    #[serde(rename = "org.opencontainers.image.created")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created: Option<String>,

    /// `org.opencontainers.image.authors`
    ///
    /// contact details of the people or organization responsible for the image (freeform string)
    #[serde(rename = "org.opencontainers.image.authors")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authors: Option<String>,

    /// `org.opencontainers.image.url`
    ///
    /// URL to find more information on the image (string)
    #[serde(rename = "org.opencontainers.image.url")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,

    /// `org.opencontainers.image.documentation`
    ///
    /// URL to get documentation on the image (string)
    #[serde(rename = "org.opencontainers.image.documentation")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub documentation: Option<String>,

    /// `org.opencontainers.image.source`
    ///
    /// URL to get source code for building the image (string)
    #[serde(rename = "org.opencontainers.image.source")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,

    /// `org.opencontainers.image.version`
    ///
    /// version of the packaged software
    #[serde(rename = "org.opencontainers.image.version")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,

    /// `org.opencontainers.image.revision`
    ///
    /// Source control revision identifier for the packaged software.
    #[serde(rename = "org.opencontainers.image.revision")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revision: Option<String>,

    /// `org.opencontainers.image.vendor`
    ///
    /// Name of the distributing entity, organization or individual.
    #[serde(rename = "org.opencontainers.image.vendor")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vendor: Option<String>,

    /// `org.opencontainers.image.licenses`
    ///
    /// License(s) under which contained software is distributed as an SPDX License Expression.
    #[serde(rename = "org.opencontainers.image.licenses")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub licenses: Option<String>,

    /// `org.opencontainers.image.ref.name`
    ///
    /// Name of the reference for a target (string).
    #[serde(rename = "org.opencontainers.image.ref.name")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ref_name: Option<String>,

    /// `org.opencontainers.image.title`
    ///
    /// Human-readable title of the image (string)
    #[serde(rename = "org.opencontainers.image.title")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,

    /// `org.opencontainers.image.description`
    ///
    /// Human-readable description of the software packaged in the image (string)
    #[serde(rename = "org.opencontainers.image.description")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// `org.opencontainers.image.base.digest`
    ///
    /// Digest of the image this image is based on (string)
    #[serde(rename = "org.opencontainers.image.base.digest")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_digest: Option<String>,

    /// `org.opencontainers.image.base.name`
    ///
    /// Annotations reference of the image this image is based on (string)
    #[serde(rename = "org.opencontainers.image.base.name")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_name: Option<String>,

    /// `io.containerd.image.name`
    ///
    /// Annotations reference of the image name
    #[serde(rename = "io.containerd.image.name")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub containerd_image_name: Option<String>,
}

impl Annotations {
    pub fn from_map(annotations: HashMap<String, String>) -> Result<Self> {
        let map = serde_json::Map::from_iter(
            annotations
                .into_iter()
                .map(|(key, value)| (key, value.into())),
        );
        Ok(serde_json::from_value(map.into())?)
    }
}

impl std::iter::FromIterator<(String, String)> for Annotations {
    fn from_iter<T>(iter: T) -> Self
    where
        T: IntoIterator<Item = (String, String)>,
    {
        let map =
            serde_json::Map::from_iter(iter.into_iter().map(|(key, value)| (key, value.into())));
        serde_json::from_value(map.into()).unwrap()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn to_map() {
        let _a = Annotations {
            url: Some("https://github.com/termoshtt/ocipkg".to_string()),
            ..Default::default()
        };
        // assert_eq!(
        //     a.into(),
        //     maplit::hashmap!(
        //         "org.opencontainers.image.url".to_string()
        //         => "https://github.com/termoshtt/ocipkg".to_string(),
        //     )
        // );
    }
}
