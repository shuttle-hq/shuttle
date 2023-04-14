#[cfg(feature = "backend")]
pub mod backends;
#[cfg(feature = "claims")]
pub mod claims;
pub mod database;
#[cfg(feature = "service")]
pub mod deployment;
#[cfg(feature = "service")]
pub mod log;
#[cfg(feature = "models")]
pub mod models;
#[cfg(feature = "service")]
pub mod project;
pub mod resource;
#[cfg(feature = "service")]
pub mod storage_manager;
#[cfg(feature = "tracing")]
pub mod tracing;
#[cfg(feature = "wasm")]
pub mod wasm;

use std::collections::BTreeMap;

use anyhow::bail;
use serde::{Deserialize, Serialize};
#[cfg(feature = "service")]
use uuid::Uuid;

#[cfg(feature = "service")]
pub use log::Item as LogItem;
#[cfg(feature = "service")]
pub use log::STATE_MESSAGE;

#[cfg(debug_assertions)]
pub const API_URL_DEFAULT: &str = "http://localhost:8001";

#[cfg(not(debug_assertions))]
pub const API_URL_DEFAULT: &str = "https://api.shuttle.rs";

pub type ApiUrl = String;
pub type Host = String;
#[cfg(feature = "service")]
pub type DeploymentId = Uuid;

#[derive(Serialize, Deserialize, Clone)]
pub struct ApiKey(String);

impl ApiKey {
    pub fn parse(key: &str) -> anyhow::Result<Self> {
        let key = key.trim().to_string();

        let mut errors = vec![];
        if !key.chars().all(char::is_alphanumeric) {
            errors.push("The API key should consist of only alphanumeric characters.");
        };

        if key.len() != 16 {
            errors.push("The API key should be exactly 16 characters in length.");
        };

        if !errors.is_empty() {
            let message = errors.join("\n");
            bail!("Invalid API key:\n{message}")
        }

        Ok(Self(key))
    }
}

impl AsRef<str> for ApiKey {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[cfg(feature = "error")]
/// Errors that can occur when changing types. Especially from prost
#[derive(thiserror::Error, Debug)]
pub enum ParseError {
    #[error("failed to parse UUID: {0}")]
    Uuid(#[from] uuid::Error),
    #[error("failed to parse timestamp: {0}")]
    Timestamp(#[from] prost_types::TimestampError),
    #[error("failed to parse serde: {0}")]
    Serde(#[from] serde_json::Error),
}

/// Holds the input for a DB resource
#[derive(Deserialize, Serialize, Default)]
pub struct DbInput {
    pub local_uri: Option<String>,
}

/// Holds the output for a DB resource
#[derive(Deserialize, Serialize)]
pub enum DbOutput {
    Info(DatabaseReadyInfo),
    Local(String),
}

/// Holds the details for a database connection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseReadyInfo {
    engine: String,
    role_name: String,
    role_password: String,
    database_name: String,
    port: String,
    address_private: String,
    address_public: String,
}

impl DatabaseReadyInfo {
    pub fn new(
        engine: String,
        role_name: String,
        role_password: String,
        database_name: String,
        port: String,
        address_private: String,
        address_public: String,
    ) -> Self {
        Self {
            engine,
            role_name,
            role_password,
            database_name,
            port,
            address_private,
            address_public,
        }
    }
    pub fn connection_string_private(&self) -> String {
        format!(
            "{}://{}:{}@{}:{}/{}",
            self.engine,
            self.role_name,
            self.role_password,
            self.address_private,
            self.port,
            self.database_name
        )
    }
    pub fn connection_string_public(&self) -> String {
        format!(
            "{}://{}:{}@{}:{}/{}",
            self.engine,
            self.role_name,
            self.role_password,
            self.address_public,
            self.port,
            self.database_name
        )
    }
}

/// Store that holds all the secrets available to a deployment
#[derive(Deserialize, Serialize, Clone)]
pub struct SecretStore {
    pub(crate) secrets: BTreeMap<String, String>,
}

impl SecretStore {
    pub fn new(secrets: BTreeMap<String, String>) -> Self {
        Self { secrets }
    }

    pub fn get(&self, key: &str) -> Option<String> {
        self.secrets.get(key).map(ToOwned::to_owned)
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use crate::ApiKey;

    proptest! {
        #[test]
        // The API key should be a 16 character alphanumeric string.
        fn parses_valid_keys(s in "[a-zA-Z0-9]{16}") {
            println!("s: {s}, len: {}", s.len());
            ApiKey::parse(&s).unwrap();
        }
    }

    #[test]
    #[should_panic(expected = "The API key should be exactly 16 characters in length.")]
    fn invalid_length() {
        ApiKey::parse("tooshort").unwrap();
    }

    #[test]
    #[should_panic(expected = "The API key should consist of only alphanumeric characters.")]
    fn non_alphanumeric() {
        ApiKey::parse("dh9z58jttoes3qv@").unwrap();
    }
}
