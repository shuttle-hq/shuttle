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
use std::fmt::Debug;
use std::fmt::Display;

use anyhow::bail;
#[cfg(feature = "service")]
pub use log::Item as LogItem;
#[cfg(feature = "service")]
pub use log::STATE_MESSAGE;
use serde::{Deserialize, Serialize};
#[cfg(feature = "service")]
use uuid::Uuid;

#[cfg(debug_assertions)]
pub const API_URL_DEFAULT: &str = "http://localhost:8001";

#[cfg(not(debug_assertions))]
pub const API_URL_DEFAULT: &str = "https://api.shuttle.rs";

pub type ApiUrl = String;
pub type Host = String;
#[cfg(feature = "service")]
pub type DeploymentId = Uuid;

#[derive(Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "persist", derive(sqlx::Type, PartialEq, Hash, Eq))]
#[cfg_attr(feature = "persist", serde(transparent))]
#[cfg_attr(feature = "persist", sqlx(transparent))]
pub struct ApiKey(String);

impl ApiKey {
    pub fn parse(key: &str) -> anyhow::Result<Self> {
        let key = key.trim();

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

        Ok(Self(key.to_string()))
    }

    #[cfg(feature = "persist")]
    pub fn generate() -> Self {
        use rand::distributions::{Alphanumeric, DistString};

        Self(Alphanumeric.sample_string(&mut rand::thread_rng(), 16))
    }
}

impl AsRef<str> for ApiKey {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

// Ensure we can't accidentaly log an ApiKey
impl Debug for ApiKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ApiKey: REDACTED")
    }
}

// Ensure we can't accidentaly log an ApiKey
impl Display for ApiKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
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

/// Holds the input for a DB resource
#[derive(Deserialize, Serialize, Default)]
pub struct DynamoDBInput {
    pub local_uri: Option<String>,
}

/// Holds the output for a DB resource
#[derive(Deserialize, Serialize)]
pub enum DynamoDBOutput {
    Info(DynamoDbReadyInfo),
    Local(String), //TODO: may remove this for local support, just need to add endpoint to DynamoDbReadyInfo
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DynamoDbReadyInfo {
    pub prefix: String,
    pub aws_access_key_id: String,
    pub aws_secret_access_key: String,
    pub aws_default_region: String,
    pub endpoint: Option<String>,
}

impl DynamoDbReadyInfo {
    pub fn new(
        prefix: String,
        aws_access_key_id: String,
        aws_secret_access_key: String,
        aws_default_region: String,
        endpoint: Option<String>,
    ) -> Self {
        Self {
            prefix,
            aws_access_key_id,
            aws_secret_access_key,
            aws_default_region,
            endpoint,
        }
    }
}

pub async fn delete_dynamodb_tables_by_prefix(dynamodb_client: &aws_sdk_dynamodb::Client, prefix: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut last_evaluated_table_name: Option<String> = Some(prefix.to_string());

    'outer: while let Some(table_name) = last_evaluated_table_name {
        let result = dynamodb_client.list_tables().exclusive_start_table_name(table_name).send().await?;
        last_evaluated_table_name = result.last_evaluated_table_name.clone();

        if let Some(table_names) = result.table_names {
            for table_name in table_names {
                if !table_name.starts_with(&prefix) {
                    break 'outer;
                } else {
                    dynamodb_client.delete_table().table_name(table_name).send().await?;
                }
            }
        }
    }

    // edge case to include just the prefix table name (if the user put only prefix for table name)
    // failure ok if no table found
    let _ = dynamodb_client.delete_table().table_name(prefix).send().await;

    Ok(())
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
        fn parses_valid_api_keys(s in "[a-zA-Z0-9]{16}") {
            ApiKey::parse(&s).unwrap();
        }
    }

    #[test]
    fn generated_api_key_is_valid() {
        let key = ApiKey::generate();

        assert!(ApiKey::parse(key.as_ref()).is_ok());
    }

    #[test]
    #[should_panic(expected = "The API key should be exactly 16 characters in length.")]
    fn invalid_api_key_length() {
        ApiKey::parse("tooshort").unwrap();
    }

    #[test]
    #[should_panic(expected = "The API key should consist of only alphanumeric characters.")]
    fn non_alphanumeric_api_key() {
        ApiKey::parse("dh9z58jttoes3qv@").unwrap();
    }
}
