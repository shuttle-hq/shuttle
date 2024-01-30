use std::str::FromStr;

use axum::{extract::State, Json};
use shuttle_common::models::error::axum::CustomErrorPath;
use shuttle_common::models::project::ProjectName;

use crate::deployer::dal::Dal;
use crate::project::ContainerInspectResponseExt;

use super::super::error::{Error, Result};
use super::{authz::ScopedProject, DeployerApiState};

pub(crate) async fn get_service(
    _scoped_project: ScopedProject,
    State(DeployerApiState { service, .. }): State<DeployerApiState>,
    CustomErrorPath((project_name, service_name)): CustomErrorPath<(String, String)>,
) -> Result<Json<shuttle_common::models::service::Summary>> {
    if let Some(user_service) = service.db.get_service_by_name(&service_name).await? {
        let deployment = service
            .db
            .get_active_deployment(&user_service.id)
            .await?
            .map(Into::into);

        let project_name = ProjectName::from_str(project_name.as_str()).map_err(Error::from)?;
        let project = service
            .find_project(&project_name)
            .await
            .map_err(Error::from)?;
        let proxy_fqdn = project
            .state
            .container()
            .ok_or(Error::ProxyFqdnMissing(
                "Project state container missing the proxy fqdn information".to_string(),
            ))?
            .fqdn()
            .map_err(|err| {
                Error::ProxyFqdnMissing(format!("Project is in errored state: {err}"))
            })?;

        let response = shuttle_common::models::service::Summary {
            uri: format!("https://{proxy_fqdn}"),
            name: user_service.name,
            deployment,
        };

        Ok(Json(response))
    } else {
        Err(Error::NotFound("service not found".to_string()))
    }
}
