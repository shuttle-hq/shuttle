mod args;
mod dal;
mod secrets;
mod session;
mod user;

use anyhow::anyhow;
use async_trait::async_trait;
use cookie::{Cookie, SameSite};
use http::header::SET_COOKIE;
use ring::rand::SystemRandom;
use secrets::KeyManager;
use session::{
    sign_cookie, SessionState, SessionToken, SessionUser, COOKIE_EXPIRATION, COOKIE_NAME,
};
use shuttle_common::claims::Claim;
use shuttle_common::ApiKey;
use shuttle_proto::auth::auth_server::Auth;
use shuttle_proto::auth::{
    ApiKeyRequest, ConvertCookieRequest, LogoutRequest, NewUser, PublicKeyRequest,
    PublicKeyResponse, ResultResponse, TokenResponse, UserRequest, UserResponse,
};
use sqlx::migrate::Migrator;
use thiserror::Error;
use tonic::metadata::MetadataValue;
use tonic::{Extensions, Request, Response, Status};
use tracing::instrument;
use user::{verify_admin, User};

use crate::dal::DalError;

pub use args::{Args, Commands, InitArgs};
pub use dal::{Dal, Sqlite};
pub use secrets::EdDsaManager;
pub use session::SessionLayer;
pub use user::AccountTier;

pub static MIGRATIONS: Migrator = sqlx::migrate!("./migrations");

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("user could not be found")]
    UserNotFound,
    #[error("{0} is not a valid account tier")]
    InvalidAccountTier(String),
    #[error("unauthorized")]
    Unauthorized,
    #[error("failed to interact with database: {0}")]
    Dal(#[from] DalError),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

pub struct Service<D, K> {
    dal: D,
    key_manager: K,
    random: SystemRandom,
    cookie_secret: cookie::Key,
}

impl<D, K> Service<D, K>
where
    D: Dal + Send + Sync + 'static,
    K: KeyManager + Send + Sync + 'static,
{
    pub fn new(dal: D, key_manager: K, random: SystemRandom, cookie_secret: cookie::Key) -> Self {
        Self {
            dal,
            key_manager,
            random,
            cookie_secret,
        }
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
        let key = ApiKey::generate();

        let user = self
            .dal
            .create_user(account_name.into(), key, account_tier.try_into()?)
            .await?;

        Ok(user)
    }

    /// Reset a users API-key, returning nothing on success. To get the new key the
    /// user will need to login on the website.
    async fn reset_key(&self, request: ApiKeyRequest) -> Result<(), Error> {
        let key = ApiKey::parse(&request.api_key)?;

        let account_name = self.dal.get_user_by_key(key).await?.name;

        let new_key = ApiKey::generate();

        self.dal.update_api_key(account_name, new_key).await?;

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
            .into_token(self.key_manager.private_key())
            .map_err(|_| anyhow!("internal server error when attempting to encode claim"))?;

        Ok(token)
    }

    /// Convert a cookie to a JWT.
    async fn convert_cookie(&self, extensions: &mut Extensions) -> Result<String, Error> {
        let SessionUser {
            account_name,
            account_tier,
        } = extensions
            .get_mut::<SessionState<D>>()
            .ok_or(Error::Unauthorized)?
            .user()
            .await
            .ok_or(Error::Unauthorized)?;

        let claim = Claim::new(account_name.to_string(), (*account_tier).into());

        let token = claim
            .into_token(self.key_manager.private_key())
            .map_err(|_| anyhow!("internal server error when attempting to encode claim"))?;

        Ok(token)
    }

    /// Get the public key for decoding JWTs.
    async fn public_key(&self) -> Vec<u8> {
        self.key_manager.public_key().to_vec()
    }

    /// Login user
    async fn login(&self, account_name: String) -> Result<(User, SessionToken), Error> {
        let user = self.dal.get_user_by_name(account_name.into()).await?;

        let session_token = SessionToken::generate(&self.random);

        self.dal.insert_session(&user.name, session_token).await?;

        Ok((user, session_token))
    }

    // Destroy callers session.
    async fn destroy_session(&self, extensions: &mut Extensions) -> Result<(), Error> {
        let token = extensions
            .get_mut::<SessionState<D>>()
            .ok_or(Error::Unauthorized)?
            .token();

        self.dal
            .delete_session(&token.into_database_value())
            .await?;

        Ok(())
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
    /// This endpoint can only be called by admin scoped users
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
            // TODO: change this to .expose() when #925 is merged.
            key: key.as_ref().to_string(),
        }))
    }

    /// Create a new user
    ///
    /// This endpoint can only be called by admin scoped users
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
            // TODO: change this to .expose() when #925 is merged.
            key: key.as_ref().to_string(),
        }))
    }

    /// Login a user
    async fn login(&self, request: Request<UserRequest>) -> Result<Response<UserResponse>, Status> {
        let request = request.into_inner();

        let (
            User {
                account_tier,
                key,
                name,
            },
            token,
        ) = self
            .login(request.account_name)
            .await
            .map_err(|err| Status::internal(err.to_string()))?;

        let mut response = Response::new(UserResponse {
            account_name: name.to_string(),
            key: key.to_string(),
            account_tier: account_tier.to_string(),
        });

        let mut cookie = Cookie::build(COOKIE_NAME, token.into_cookie_value())
            .secure(true)
            .http_only(true)
            .same_site(SameSite::Strict)
            .path("/")
            .expires(Some(
                (std::time::SystemTime::now() + COOKIE_EXPIRATION).into(),
            ))
            .finish();

        sign_cookie(&self.cookie_secret, &mut cookie);

        response.metadata_mut().insert(
            SET_COOKIE.as_str(),
            MetadataValue::try_from(&cookie.to_string())
                .expect("cookie should not contain invalid metadata value characters"),
        );

        Ok(response)
    }

    /// Logout a user
    async fn logout(
        &self,
        mut request: Request<LogoutRequest>,
    ) -> Result<Response<ResultResponse>, Status> {
        // Logout user
        _ = self.destroy_session(request.extensions_mut()).await;

        let mut cookie = Cookie::build(COOKIE_NAME, "")
            .http_only(true)
            .path("/")
            .finish();

        cookie.make_removal();

        sign_cookie(&self.cookie_secret, &mut cookie);

        let mut response = Response::new(ResultResponse {
            success: true,
            ..Default::default()
        });

        response.metadata_mut().insert(
            SET_COOKIE.as_str(),
            MetadataValue::try_from(&cookie.to_string())
                .expect("cookie should not contain invalid metadata value characters"),
        );

        Ok(response)
    }

    /// Convert an API key to a JWT
    async fn convert_api_key(
        &self,
        request: Request<ApiKeyRequest>,
    ) -> Result<Response<TokenResponse>, Status> {
        let request = request.into_inner();

        let token = self
            .convert_key(request)
            .await
            .map_err(|err| Status::permission_denied(err.to_string()))?;

        Ok(Response::new(TokenResponse { token }))
    }

    /// Convert a cookie to a JWT
    async fn convert_cookie(
        &self,
        mut request: Request<ConvertCookieRequest>,
    ) -> Result<Response<TokenResponse>, Status> {
        let token = self
            .convert_cookie(request.extensions_mut())
            .await
            .map_err(|err| Status::permission_denied(err.to_string()))?;

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
