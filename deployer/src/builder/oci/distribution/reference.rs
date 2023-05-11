use super::super::error::*;
use once_cell::sync::Lazy;
use regex::Regex;
use std::fmt;

/// Reference of container image stored in the repository
///
/// In [OCI distribution spec](https://github.com/opencontainers/distribution-spec/blob/main/spec.md):
/// > `<reference>` as a tag MUST be at most 128 characters
/// > in length and MUST match the following regular expression:
/// > ```text
/// > [a-zA-Z0-9_][a-zA-Z0-9._-]{0,127}
/// > ```
/// This struct checks this restriction at creation.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Reference(pub String);

static REF_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^[a-zA-Z0-9_][a-zA-Z0-9._-]{0,127}$").expect("to create a regex from pattern")
});

impl std::ops::Deref for Reference {
    type Target = str;
    fn deref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Reference {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Reference {
    pub fn new(name: &str) -> Result<Self> {
        if REF_RE.is_match(name) {
            Ok(Reference(name.to_string()))
        } else {
            Err(Error::InvalidReference(name.to_string()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reference() {
        assert!(Reference::new("latest").is_ok());
        // @ is not allowed
        assert!(Reference::new("my_super_tag@2").is_err());
    }
}
