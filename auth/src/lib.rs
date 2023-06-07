mod api;
mod args;
mod dal;
mod secrets;
mod user;

use std::error::Error as StdError;
use std::time::Duration;

use args::StartArgs;
use async_trait::async_trait;
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::IntoResponse;
use axum::Json;
use dal::Dal;
use secrets::KeyManager;
use serde::{ser::SerializeMap, Serialize};
use shuttle_common::claims::Claim;
use shuttle_common::models::error::ApiError;
use shuttle_common::ApiKey;
use shuttle_proto::auth::auth_server::Auth;
use shuttle_proto::auth::{
    ApiKeyRequest, NewUser, PublicKeyRequest, PublicKeyResponse, ResultResponse, TokenResponse,
    UserRequest, UserResponse,
};
use sqlx::migrate::Migrator;
use thiserror::Error;
use tonic::{Request, Response, Status};
use tracing::{info, instrument};
use user::User;

use crate::api::serve;
use crate::dal::DalError;

pub use api::ApiBuilder;
pub use args::{Args, Commands, InitArgs};
pub use dal::Sqlite;

pub const COOKIE_EXPIRATION: Duration = Duration::from_secs(60 * 60 * 24); // One day

pub static MIGRATIONS: Migrator = sqlx::migrate!("./migrations");

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("user could not be found")]
    UserNotFound,
    #[error("{0} is not a valid account tier")]
    InvalidAccountTier(String),
    #[error("API key is missing")]
    KeyMissing,
    #[error("unauthorized")]
    Unauthorized,
    #[error("forbidden")]
    Forbidden,
    #[error("failed to interact with database: {0}")]
    Dal(#[from] DalError),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl Serialize for Error {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut map = serializer.serialize_map(Some(2))?;
        map.serialize_entry("type", &format!("{:?}", self))?;
        // use the error source if available, if not use display implementation
        map.serialize_entry("msg", &self.source().unwrap_or(self).to_string())?;
        map.end()
    }
}

impl IntoResponse for Error {
    fn into_response(self) -> axum::response::Response {
        let code = match self {
            Error::Forbidden => StatusCode::FORBIDDEN,
            Error::Unauthorized | Error::KeyMissing => StatusCode::UNAUTHORIZED,
            Error::Dal(_) | Error::UserNotFound => StatusCode::NOT_FOUND,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };

        (
            code,
            [(
                header::CONTENT_TYPE,
                HeaderValue::from_static("application/json"),
            )],
            Json(ApiError {
                message: self.to_string(),
                status_code: code.as_u16(),
            }),
        )
            .into_response()
    }
}

pub async fn start(sqlite: Sqlite, args: StartArgs) {
    let router = api::ApiBuilder::new()
        .with_sqlite(sqlite)
        .with_sessions()
        .into_router();

    info!(address=%args.address, "Binding to and listening at address");

    serve(router, args.address).await;
}

pub struct Service<D, K> {
    dal: D,
    key_manager: K,
}

impl<D, K> Service<D, K>
where
    D: Dal + Send + Sync + 'static,
    K: KeyManager + Send + Sync + 'static,
{
    pub fn new(dal: D, key_manager: K) -> Self {
        Self { dal, key_manager }
    }

    /// Get a user from the database.
    #[instrument(skip(self))]
    async fn get_user(&self, account_name: String) -> Result<User, Error> {
        let user = self.dal.get_user(account_name.into()).await?;

        Ok(user)
    }

    /// Insert a new user into the database.
    #[instrument(skip(self))]
    async fn post_user(
        &self,
        NewUser {
            account_name,
            account_tier,
        }: NewUser,
    ) -> Result<User, Error> {
        let user = self
            .dal
            .create_user(account_name.into(), account_tier.try_into()?)
            .await?;

        Ok(user)
    }

    /// Reset a users API-key, returning nothing on success. To get the new key the
    /// user will need to login on the website.
    async fn put_user_reset_key(&self, key: ApiKey) -> Result<(), Error> {
        let account_name = self.dal.get_user_by_key(key).await?.name;

        self.dal.reset_key(account_name).await?;

        Ok(())
    }

    /// Convert a valid API-key bearer token to a JWT.
    async fn convert_key(&self, key: ApiKey) -> Result<String, Error> {
        let User {
            name, account_tier, ..
        } = self
            .dal
            .get_user_by_key(key)
            .await
            .map_err(|_| Error::Unauthorized)?;

        let claim = Claim::new(name.to_string(), account_tier.into());

        let token = claim
            .into_token(self.key_manager.private_key())
            // TODO: refactor .into_token error handling?
            .map_err(|_| Error::Unauthorized)?;

        Ok(token)
    }

    /// Get the public key for decoding JWTs.
    async fn get_public_key(&self) -> Vec<u8> {
        self.key_manager.public_key().to_vec()
    }

    async fn refresh_token() {}
}

#[async_trait]
impl<D, K> Auth for Service<D, K>
where
    D: Dal + Send + Sync + 'static,
    K: KeyManager + Send + Sync + 'static,
{
    /// Get a user
    async fn get_user_request(
        &self,
        req: Request<UserRequest>,
    ) -> Result<Response<UserResponse>, Status> {
        todo!()
    }

    /// Create a new user
    async fn post_user_request(
        &self,
        req: Request<NewUser>,
    ) -> Result<Response<UserResponse>, Status> {
        todo!()
    }

    /// Convert an API key to a JWT
    async fn convert_api_key(
        &self,
        req: Request<ApiKeyRequest>,
    ) -> Result<Response<TokenResponse>, Status> {
        todo!()
    }

    /// Reset a users API key
    async fn reset_api_key(
        &self,
        req: Request<ApiKeyRequest>,
    ) -> Result<Response<ResultResponse>, Status> {
        todo!()
    }

    /// Get the auth service public key to decode tokens
    async fn public_key(
        &self,
        req: Request<PublicKeyRequest>,
    ) -> Result<Response<PublicKeyResponse>, Status> {
        todo!()
    }
}
