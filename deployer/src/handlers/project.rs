use std::collections::HashMap;

use async_trait::async_trait;
use axum::extract::{FromRequest, Path, RequestParts};
use hyper::StatusCode;
use shuttle_common::project::ProjectName;
use tracing::error;

/// Gaurd to ensure request are for the project served by this deployer
/// Note: this guard needs the `ProjectName` extension to be set
pub struct ProjectNameGuard;

#[async_trait]
impl<B> FromRequest<B> for ProjectNameGuard
where
    B: Send,
{
    type Rejection = StatusCode;

    async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
        // We expect some path parameters
        let Path(path): Path<HashMap<String, String>> = match req.extract().await {
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
        let expected_project_name: &ProjectName = match req.extensions().get() {
            Some(expected) => expected,
            None => {
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
