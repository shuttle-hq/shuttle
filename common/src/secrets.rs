use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fmt::{Debug, Display},
};
use zeroize::Zeroize;

/// Wrapper type for secret values such as passwords or authentication keys.
///
/// Once wrapped, the inner value cannot leak accidentally, as both the [`Display`] and [`Debug`]
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

impl<T: Zeroize> Display for Secret<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
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

    pub fn expose(&self) -> &T {
        &self.0
    }
}

/// Store that holds all the secrets available to a deployment
#[derive(Deserialize, Serialize, Clone)]
pub struct SecretStore {
    pub(crate) secrets: BTreeMap<String, Secret<String>>,
}

impl SecretStore {
    pub fn new(secrets: BTreeMap<String, Secret<String>>) -> Self {
        Self { secrets }
    }

    pub fn get(&self, key: &str) -> Option<String> {
        self.secrets
            .get(key)
            .map(|secret| secret.expose().to_owned())
    }
}

#[cfg(test)]
#[allow(dead_code)]
mod secrets_tests {
    use super::*;

    #[test]
    fn display() {
        let password_string = String::from("VERYSECRET");
        let secret = Secret::new(password_string);
        let printed = format!("{}", secret);
        assert_eq!(printed, "[REDACTED \"alloc::string::String\"]");
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
