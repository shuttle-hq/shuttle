use chrono::{DateTime, Utc};
use ulid::Ulid;

#[async_trait::async_trait]
/// Record a secret value for a service with name
pub trait SecretRecorder: Clone + Send + Sync + 'static {
    type Err: std::error::Error + Send;

    async fn insert_secret(
        &self,
        service_id: &Ulid,
        key: &str,
        value: &str,
    ) -> Result<(), Self::Err>;
}

#[async_trait::async_trait]
/// Get all the secrets for the service with the given name
pub trait SecretGetter: Clone + Send + Sync + 'static {
    type Err: std::error::Error + Send + Sync;

    async fn get_secrets(&self, service_id: &Ulid) -> Result<Vec<Secret>, Self::Err>;
}

#[derive(sqlx::FromRow, Debug, Eq, PartialEq)]
pub struct Secret {
    pub service_id: Ulid,
    pub key: String,
    pub value: String,
    pub last_update: DateTime<Utc>,
}

impl From<Secret> for shuttle_common::models::secret::Response {
    fn from(secret: Secret) -> Self {
        Self {
            key: secret.key,
            last_update: secret.last_update,
        }
    }
}
