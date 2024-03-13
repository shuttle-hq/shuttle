use crate::{
    error::Error,
    user::{Admin, Key},
};
use axum::{
    extract::{Path, State},
    Json,
};
use http::StatusCode;
use shuttle_common::{
    claims::{AccountTier, Claim},
    models::user::{self, SubscriptionRequest, UserId},
};
use tracing::{field, instrument, Span};

use super::{
    builder::{KeyManagerState, UserManagerState},
    RouterState,
};

#[instrument(skip_all, fields(account.user_id = %user_id))]
pub(crate) async fn get_user(
    _: Admin,
    State(user_manager): State<UserManagerState>,
    Path(user_id): Path<UserId>,
) -> Result<Json<user::Response>, Error> {
    let user = user_manager.get_user(user_id).await?;

    Ok(Json(user.into()))
}

#[instrument(skip_all, fields(account.name = %account_name, account.user_id = field::Empty))]
pub(crate) async fn get_user_by_name(
    _: Admin,
    State(user_manager): State<UserManagerState>,
    Path(account_name): Path<String>,
) -> Result<Json<user::Response>, Error> {
    let user = user_manager.get_user_by_name(&account_name).await?;
    Span::current().record("account.user_id", &user.id);

    Ok(Json(user.into()))
}

#[instrument(skip_all, fields(account.name = %account_name, account.tier = %account_tier, account.user_id = field::Empty))]
pub(crate) async fn post_user(
    _: Admin,
    State(user_manager): State<UserManagerState>,
    Path((account_name, account_tier)): Path<(String, AccountTier)>,
) -> Result<Json<user::Response>, Error> {
    let user = user_manager.create_user(account_name, account_tier).await?;
    Span::current().record("account.user_id", &user.id);

    Ok(Json(user.into()))
}

pub(crate) async fn put_user_reset_key(
    State(user_manager): State<UserManagerState>,
    key: Key,
) -> Result<(), Error> {
    let user_id = user_manager.get_user_by_key(key.into()).await?.id;

    user_manager.reset_key(user_id).await
}

pub(crate) async fn post_subscription(
    _: Admin,
    State(user_manager): State<UserManagerState>,
    Path(user_id): Path<UserId>,
    payload: Json<SubscriptionRequest>,
) -> Result<(), Error> {
    user_manager
        .insert_subscription(&user_id, &payload.id, &payload.r#type, payload.quantity)
        .await?;

    Ok(())
}

pub(crate) async fn delete_subscription(
    _: Admin,
    State(user_manager): State<UserManagerState>,
    Path((user_id, subscription_id)): Path<(UserId, String)>,
) -> Result<(), Error> {
    user_manager
        .delete_subscription(&user_id, &subscription_id)
        .await?;

    Ok(())
}

/// Convert a valid API-key bearer token to a JWT.
pub(crate) async fn convert_key(
    _: Admin,
    State(RouterState {
        key_manager,
        user_manager,
    }): State<RouterState>,
    key: Key,
) -> Result<Json<shuttle_common::backends::auth::ConvertResponse>, StatusCode> {
    let user = user_manager
        .get_user_by_key(key.into())
        .await
        .map_err(|_| StatusCode::UNAUTHORIZED)?;

    let claim = Claim::new(
        user.id.clone(),
        user.account_tier.into(),
        user.account_tier,
        user,
    );

    let token = claim.into_token(key_manager.private_key())?;

    let response = shuttle_common::backends::auth::ConvertResponse { token };

    Ok(Json(response))
}

pub(crate) async fn get_public_key(State(key_manager): State<KeyManagerState>) -> Vec<u8> {
    key_manager.public_key().to_vec()
}
