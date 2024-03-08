use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fmt::Debug, ops::Deref};
use zeroize::Zeroize;

/// Wrapper type for secret values such as passwords or authentication keys.
///
/// Once wrapped, the inner value cannot leak accidentally, as both the [`std::fmt::Display`] and [`Debug`]
/// implementations cover up the actual value and only show the type.
///
/// If you need access to the inner value, there is an [expose](`Secret::expose`) method.
///
/// To make sure nothing leaks after the [`Secret`] has been dropped, a custom [`Drop`]
/// implementation will zero-out the underlying memory.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Secret<T: Zeroize>(T);

impl<T: Zeroize> Debug for Secret<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[REDACTED {:?}]", std::any::type_name::<T>())
    }
}

impl<T: Zeroize> Drop for Secret<T> {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

impl<T: Zeroize> From<T> for Secret<T> {
    fn from(value: T) -> Self {
        Self::new(value)
    }
}

impl<T: Zeroize> Secret<T> {
    pub fn new(secret: T) -> Self {
        Self(secret)
    }

    /// Expose the underlying value of the secret
    pub fn expose(&self) -> &T {
        &self.0
    }

    /// Display a placeholder for the secret
    pub fn redacted(&self) -> &str {
        "********"
    }
}

/// Store that holds all the secrets available to a deployment
#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(transparent)]
pub struct SecretStore(pub BTreeMap<String, String>);

impl SecretStore {
    pub fn get(&self, key: &str) -> Option<String> {
        self.0.get(key).map(|s| s.to_owned())
    }
}

impl Deref for SecretStore {
    type Target = BTreeMap<String, String>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl config::Source for SecretStore {
    fn clone_into_box(&self) -> Box<dyn config::Source + Send + Sync> {
        Box::new(self.clone())
    }

    fn collect(&self) -> Result<config::Map<String, config::Value>, config::ConfigError> {
        let mut map: config::Map<String, config::Value> = config::Map::new();
        for (a, b) in self.iter() {
            map.insert(a.clone(), b.clone().into());
        }
        Ok(map)
    }
}

#[cfg(test)]
#[allow(dead_code)]
mod secrets_tests {
    use super::*;

    #[test]
    fn redacted() {
        let password_string = String::from("VERYSECRET");
        let secret = Secret::new(password_string);
        assert_eq!(secret.redacted(), "********");
    }

    #[test]
    fn debug() {
        let password_string = String::from("VERYSECRET");
        let secret = Secret::new(password_string);
        let printed = format!("{:?}", secret);
        assert_eq!(printed, "[REDACTED \"alloc::string::String\"]");
    }

    #[test]
    fn expose() {
        let password_string = String::from("VERYSECRET");
        let secret = Secret::new(password_string);
        let printed = secret.expose();
        assert_eq!(printed, "VERYSECRET");
    }

    #[test]
    fn secret_struct() {
        #[derive(Debug)]
        struct Wrapper {
            password: Secret<String>,
        }

        let password_string = String::from("VERYSECRET");
        let secret = Secret::new(password_string);
        let wrapper = Wrapper { password: secret };
        let printed = format!("{:?}", wrapper);
        assert_eq!(
            printed,
            "Wrapper { password: [REDACTED \"alloc::string::String\"] }"
        );
    }
}
