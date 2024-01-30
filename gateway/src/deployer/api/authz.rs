use axum::extract::{FromRef, FromRequestParts};
use axum::headers::HeaderMapExt;
use axum::http::request::Parts;
use shuttle_common::backends::headers::XShuttleAdminSecret;
use shuttle_common::models::project::ProjectName;
use tracing::Span;

use super::DeployerApiState;
use crate::deployer::dal::Dal;
use crate::{Error, ErrorKind};

/// This needs to be used for every API request handler that will be
/// authorized through the admin secret, set as a deployer flag.
pub struct ScopedProject(pub ProjectName);

#[async_trait]
impl<S> FromRequestParts<S> for ScopedProject
where
    S: Send + Sync,
    DeployerApiState: FromRef<S>,
{
    type Rejection = Error;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let project_name = match parts.headers.typed_try_get::<XShuttleAdminSecret>() {
            Ok(Some(secret)) => {
                let deployer_api_state = DeployerApiState::from_ref(state);
                // For this particular case, we expect the secret to be the deployer admin secret.
                deployer_api_state
                    .service
                    .db
                    .project_name_by_admin_secret(secret.0.as_str())
                    .await
                    .map_err(|_| {
                        Error::custom(
                            ErrorKind::Internal,
                            "Couldn't validate the authority of the deployer request",
                        )
                    })?
                    .ok_or(Error::custom(
                        ErrorKind::Unauthorized,
                        "Authority check failed",
                    ))
            }
            Ok(_) => Err(Error::custom(
                ErrorKind::Unauthorized,
                "Authority check failed",
            )),
            // Returning forbidden for the cases where we don't understand why we can not authorize.
            Err(_) => Err(Error::custom(
                ErrorKind::Forbidden,
                "We can not authorize this request",
            )),
        }?;

        // Record current project name for tracing purposes
        Span::current().record("shuttle.project.name", project_name.to_string());

        Ok(ScopedProject(project_name))
    }
}
