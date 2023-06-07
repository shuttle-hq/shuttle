mod api;
mod args;
mod dal;
mod secrets;
mod user;

use std::time::Duration;

use async_trait::async_trait;
use dal::Dal;
use secrets::KeyManager;
use shuttle_common::claims::Claim;
use shuttle_common::ApiKey;
use shuttle_proto::auth::auth_server::Auth;
use shuttle_proto::auth::{
    ApiKeyRequest, NewUser, PublicKeyRequest, PublicKeyResponse, ResultResponse, TokenResponse,
    UserRequest, UserResponse,
};
use sqlx::migrate::Migrator;
use thiserror::Error;
use tonic::{Request, Response, Status};
use tracing::instrument;
use user::{verify_admin, User};

use crate::dal::DalError;

pub use args::{Args, Commands, InitArgs};
pub use dal::Sqlite;
pub use secrets::EdDsaManager;

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
        let user = self.dal.get_user_by_name(account_name.into()).await?;

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
    async fn reset_key(&self, request: ApiKeyRequest) -> Result<(), Error> {
        let key = ApiKey::parse(&request.api_key)?;

        let account_name = self.dal.get_user_by_key(key).await?.name;

        self.dal.reset_api_key(account_name).await?;

        Ok(())
    }

    /// Convert a valid API-key bearer token to a JWT.
    async fn convert_key(&self, request: ApiKeyRequest) -> Result<String, Error> {
        let key = ApiKey::parse(&request.api_key)?;

        let User {
            name, account_tier, ..
        } = self
            .dal
            .get_user_by_key(key)
            .await
            .map_err(|_| Error::UserNotFound)?;

        let claim = Claim::new(name.to_string(), account_tier.into());

        let token = claim
            .into_token(self.key_manager.get_private_key())
            // TODO: error handling
            .map_err(|_| Error::Unauthorized)?;

        Ok(token)
    }

    /// Get the public key for decoding JWTs.
    async fn public_key(&self) -> Vec<u8> {
        self.key_manager.get_public_key().to_vec()
    }
}

#[async_trait]
impl<D, K> Auth for Service<D, K>
where
    D: Dal + Send + Sync + 'static,
    K: KeyManager + Send + Sync + 'static,
{
    /// Get a user
    ///
    /// **This endpoint can only be called by admin scoped users**
    async fn get_user_request(
        &self,
        request: Request<UserRequest>,
    ) -> Result<Response<UserResponse>, Status> {
        verify_admin(request.metadata(), &self.dal).await?;

        let request = request.into_inner();

        let User {
            account_tier,
            key,
            name,
        } = self
            .get_user(request.account_name)
            .await
            .map_err(|err| Status::not_found(err.to_string()))?;

        Ok(Response::new(UserResponse {
            account_name: name.to_string(),
            account_tier: account_tier.to_string(),
            key: key.to_string(),
        }))
    }

    /// Create a new user
    ///
    /// **This endpoint can only be called by admin scoped users**
    async fn post_user_request(
        &self,
        request: Request<NewUser>,
    ) -> Result<Response<UserResponse>, Status> {
        verify_admin(request.metadata(), &self.dal).await?;

        let request = request.into_inner();

        let User {
            account_tier,
            key,
            name,
        } = self
            .post_user(request)
            .await
            .map_err(|err| Status::internal(err.to_string()))?;

        Ok(Response::new(UserResponse {
            account_name: name.to_string(),
            account_tier: account_tier.to_string(),
            key: key.to_string(),
        }))
    }

    /// Convert an API key to a JWT
    async fn convert_api_key(
        &self,
        request: Request<ApiKeyRequest>,
    ) -> Result<Response<TokenResponse>, Status> {
        let request = request.into_inner();

        // TODO: error handling upstream
        let token = self
            .convert_key(request)
            .await
            .map_err(|err| Status::internal(err.to_string()))?;

        Ok(Response::new(TokenResponse { token }))
    }

    /// Reset a users API key
    async fn reset_api_key(
        &self,
        request: Request<ApiKeyRequest>,
    ) -> Result<Response<ResultResponse>, Status> {
        let request = request.into_inner();

        let result = match self.reset_key(request).await {
            Ok(()) => ResultResponse {
                success: true,
                message: Default::default(),
            },
            Err(e) => ResultResponse {
                success: false,
                message: e.to_string(),
            },
        };

        Ok(Response::new(result))
    }

    /// Get the auth service public key to decode tokens
    async fn public_key(
        &self,
        _request: Request<PublicKeyRequest>,
    ) -> Result<Response<PublicKeyResponse>, Status> {
        let public_key = self.public_key().await;

        Ok(Response::new(PublicKeyResponse { public_key }))
    }
}
