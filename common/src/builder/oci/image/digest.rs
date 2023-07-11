use super::super::error::*;
use once_cell::sync::Lazy;
use regex::Regex;
use sha2::{Digest as _, Sha256};
use std::{fmt, path::PathBuf};

/// Digest of contents
///
/// Digest is defined in [OCI image spec](https://github.com/opencontainers/image-spec/blob/v1.0.1/descriptor.md#digests)
/// as a string satisfies following EBNF:
///
/// ```text
/// digest                ::= algorithm ":" encoded
/// algorithm             ::= algorithm-component (algorithm-separator algorithm-component)*
/// algorithm-component   ::= [a-z0-9]+
/// algorithm-separator   ::= [+._-]
/// encoded               ::= [a-zA-Z0-9=_-]+
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Digest {
    pub algorithm: String,
    pub encoded: String,
}

static ENCODED_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"[a-zA-Z0-9=_-]+").expect("to create a regex from pattern"));

impl fmt::Display for Digest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.algorithm, self.encoded)
    }
}

impl Digest {
    pub fn new(input: &str) -> Result<Self> {
        let mut iter = input.split(':');
        match (iter.next(), iter.next(), iter.next()) {
            (Some(algorithm), Some(encoded), None) => {
                // FIXME: check algorithm part
                if ENCODED_RE.is_match(encoded) {
                    Ok(Digest {
                        algorithm: algorithm.to_string(),
                        encoded: encoded.to_string(),
                    })
                } else {
                    Err(Error::InvalidDigest(input.to_string()))
                }
            }
            _ => Err(Error::InvalidDigest(input.to_string())),
        }
    }

    /// As a path used in oci-archive
    pub fn as_path(&self) -> PathBuf {
        PathBuf::from(format!("blobs/{}/{}", self.algorithm, self.encoded))
    }

    /// Calc digest using SHA-256 algorithm
    pub fn from_buf_sha256(buf: &[u8]) -> Self {
        let hash = Sha256::digest(buf);
        let digest = base16ct::lower::encode_string(&hash);
        Self {
            algorithm: "sha256".to_string(),
            encoded: digest,
        }
    }
}

#[cfg(test)]
mod tests {

    use super::Digest;

    #[test]
    fn digest_new() {
        assert!(Digest::new("sha256:%").is_err());
        assert!(Digest::new("sha256:xyz:w").is_err());
        assert!(Digest::new("sha256:xyz").is_ok());
    }

    #[test]
    fn digest_as_path() {
        let digest = Digest::new("sha256:xyz").unwrap();
        assert_eq!(
            digest.as_path().to_string_lossy().to_string(),
            format!("blobs/{}/{}", digest.algorithm, digest.encoded)
        );
    }
}
