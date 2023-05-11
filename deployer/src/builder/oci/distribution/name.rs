use super::super::error::*;
use once_cell::sync::Lazy;
use regex::Regex;
use std::fmt;

/// Namespace of the repository
///
/// In [OCI distribution spec](https://github.com/opencontainers/distribution-spec/blob/main/spec.md):
/// > `<name>` MUST match the following regular expression:
/// > ```text
/// > [a-z0-9]+([._-][a-z0-9]+)*(/[a-z0-9]+([._-][a-z0-9]+)*)*
/// > ```
/// This struct checks this restriction at creation.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Name(String);

impl std::ops::Deref for Name {
    type Target = str;
    fn deref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Name {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

static NAME_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^[a-z0-9]+([._-][a-z0-9]+)*(/[a-z0-9]+([._-][a-z0-9]+)*)*$")
        .expect("to create a regex from pattern")
});

impl Name {
    pub fn new(name: &str) -> Result<Self> {
        if NAME_RE.is_match(name) {
            Ok(Name(name.to_string()))
        } else {
            Err(Error::InvalidName(name.to_string()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name() {
        assert!(Name::new("ghcr.io").is_ok());
        // Head must be alphanum
        assert!(Name::new("_ghcr.io").is_err());
        assert!(Name::new("/ghcr.io").is_err());
    }
}
