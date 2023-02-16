use axum::{
    extract::{Path, State},
    Json,
};
use tracing::instrument;

use crate::{
    api::builder::RouterState,
    error::Error,
    user::{AccountName, Admin, User, UserManagement},
};

#[instrument(skip(user_manager))]
pub(crate) async fn get_user(
    _: Admin,
    State(RouterState { user_manager }): State<RouterState>,
    Path(account_name): Path<AccountName>,
) -> Result<Json<User>, Error> {
    let user = user_manager.get_user(account_name).await?;
    // TODO: use the user response struct in common, or create a new one
    Ok(Json(user))
}

#[instrument(skip(user_manager))]
pub(crate) async fn post_user(
    _: Admin,
    State(RouterState { user_manager }): State<RouterState>,
    Path(account_name): Path<AccountName>,
) -> Result<Json<User>, Error> {
    let user = user_manager.create_user(account_name).await?;
    // TODO: use the user response struct in common, or create a new one
    Ok(Json(user))
}

pub(crate) async fn login() {}

pub(crate) async fn logout() {}

pub(crate) async fn convert_cookie() {}

pub(crate) async fn convert_key() {}

pub(crate) async fn refresh_token() {}

pub(crate) async fn get_public_key() {}
