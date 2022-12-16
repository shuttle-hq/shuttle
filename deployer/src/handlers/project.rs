use std::collections::HashMap;

use async_trait::async_trait;
use axum::extract::{Extension, FromRequestParts, Path};
use axum::http::request::Parts;
use axum::RequestPartsExt;
use hyper::StatusCode;
use shuttle_common::project::ProjectName;
use tracing::error;

/// Gaurd to ensure request are for the project served by this deployer
/// Note: this guard needs the `ProjectName` extension to be set
pub struct ProjectNameGuard;

#[async_trait]
impl<S> FromRequestParts<S> for ProjectNameGuard
where
    S: Send + Sync,
{
    type Rejection = StatusCode;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        // We expect some path parameters
        let Path(path): Path<HashMap<String, String>> =
            match Path::from_request_parts(parts, state).await {
                Ok(path) => path,
                Err(_) => return Err(StatusCode::NOT_FOUND),
            };

        // All our routes have the `project_name` parameter
        let project_name = match path.get("project_name") {
            Some(project_name) => project_name,
            None => {
                error!("ProjectNameGuard found no project name in path");
                return Err(StatusCode::INTERNAL_SERVER_ERROR);
            }
        };

        // This extractor requires the ProjectName extension to be set
        let Extension(expected_project_name) = match parts.extract::<Extension<ProjectName>>().await
        {
            Ok(expected) => expected,
            Err(_) => {
                error!("ProjectName extension is not set");
                return Err(StatusCode::INTERNAL_SERVER_ERROR);
            }
        };

        if project_name == expected_project_name.as_str() {
            Ok(ProjectNameGuard)
        } else {
            error!(project_name, "project is not served by this deployer");
            Err(StatusCode::BAD_REQUEST)
        }
    }
}
