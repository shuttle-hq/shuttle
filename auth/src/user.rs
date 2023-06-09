use std::{fmt::Formatter, str::FromStr};

use shuttle_common::{
    claims::{Scope, ScopeBuilder},
    ApiKey,
};
use tonic::{metadata::MetadataMap, Status};

use crate::{dal::Dal, Error};

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct User {
    pub name: AccountName,
    pub key: ApiKey,
    pub account_tier: AccountTier,
}

impl User {
    pub fn is_admin(&self) -> bool {
        self.account_tier == AccountTier::Admin
    }

    pub fn new(name: AccountName, key: ApiKey, account_tier: AccountTier) -> Self {
        Self {
            name,
            key,
            account_tier,
        }
    }
}

/// Check the request metadata for the bearer token of a user with admin tier. If we cannot
/// establish that for any reason, return an error with a permission denied status.
pub async fn verify_admin<D: Dal + Send + Sync + 'static>(
    headers: &MetadataMap,
    dal: &D,
) -> Result<(), Status> {
    let err = || Status::permission_denied("Unauthorized.");

    let bearer = headers.get("authorization").ok_or_else(err)?;

    let bearer = bearer.to_str().map_err(|_| err())?;

    let (_, token) = bearer.split_once("Bearer ").ok_or_else(err)?;

    let key = ApiKey::parse(token).map_err(|_| err())?;

    // TODO: refactor to `is_ok_and(|user| user.is_admin())` when
    // 1.70 PR is merged.
    let user = dal.get_user_by_key(key).await.map_err(|_| err())?;

    if !user.is_admin() {
        Err(err())
    } else {
        Ok(())
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, sqlx::Type, strum::Display)]
#[sqlx(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
#[derive(Default)]
pub enum AccountTier {
    #[default]
    Basic,
    Pro,
    Team,
    Admin,
}

impl From<AccountTier> for Vec<Scope> {
    fn from(tier: AccountTier) -> Self {
        let mut builder = ScopeBuilder::new();

        if tier == AccountTier::Admin {
            builder = builder.with_admin()
        }

        builder.build()
    }
}

impl TryFrom<String> for AccountTier {
    type Error = Error;

    fn try_from(value: String) -> Result<AccountTier, Error> {
        let tier = match value.as_str() {
            "basic" => AccountTier::Basic,
            "pro" => AccountTier::Pro,
            "team" => AccountTier::Team,
            "admin" => AccountTier::Admin,
            other => return Err(Error::InvalidAccountTier(other.to_string())),
        };

        Ok(tier)
    }
}
#[derive(Debug, Clone, PartialEq, Eq, sqlx::Type)]
#[sqlx(transparent)]
pub struct AccountName(String);

impl From<String> for AccountName {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl FromStr for AccountName {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(s.to_string().into())
    }
}

impl std::fmt::Display for AccountName {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}
