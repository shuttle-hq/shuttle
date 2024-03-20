use std::collections::HashMap;

use http::{Method, Uri};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::{Error, ServicesApiClient};

#[derive(Clone)]
pub struct Permit {
    /// The Permit.io API
    api: ServicesApiClient,
    /// The local Permit PDP (Policy decision point) API
    pdp: ServicesApiClient,
    /// The base URL path for 'facts' endpoints. Helps with building full URLs.
    facts: String,
}

impl Permit {
    pub fn new(api_uri: Uri, pdp_uri: Uri, proj_id: String, env_id: String, api_key: &str) -> Self {
        Self {
            api: ServicesApiClient::new_with_bearer(api_uri, api_key),
            pdp: ServicesApiClient::new(pdp_uri),
            facts: format!("/v2/facts/{}/{}", proj_id, env_id),
        }
    }

    /// Creates a Project resource and assigns the user as admin for that project
    pub async fn create_project(&self, user_id: &str, project_id: &str) -> Result<(), Error> {
        self.api
            .post(
                &format!("{}/resource_instances", self.facts),
                json!({
                    "key": project_id,
                    "tenant": "default",
                    "resource": "Project",
                }),
                None,
            )
            .await?;

        self.api
            .post(
                &format!("{}/role_assignments", self.facts),
                json!({
                    "role": "admin",
                    "resource_instance": format!("Project:{project_id}"),
                    "tenant": "default",
                    "user": user_id,
                }),
                None,
            )
            .await
    }

    /// Unassigns the admin role for a user on a project
    pub async fn delete_user_project(&self, user_id: &str, project_id: &str) -> Result<(), Error> {
        self.api
            .delete(
                &format!("{}/role_assignments", self.facts),
                json!({
                    "role": "admin",
                    "resource_instance": format!("Project:{project_id}"),
                    "tenant": "default",
                    "user": user_id,
                }),
                None,
            )
            .await
    }

    /// Assigns a user to an org directly without creating the org first
    pub async fn create_organization(&self, user_id: &str, org_name: &str) -> Result<(), Error> {
        self.api
            .post(
                &format!("{}/resource_instances", self.facts),
                json!({
                    "key": org_name,
                    "tenant": "default",
                    "resource": "Organization",
                }),
                None,
            )
            .await?;

        self.api
            .post(
                &format!("{}/role_assignments", self.facts),
                json!({
                    "role": "admin",
                    "resource_instance": format!("Organization:{org_name}"),
                    "tenant": "default",
                    "user": user_id,
                }),
                None,
            )
            .await
    }

    pub async fn delete_organization(&self, organization_id: &str) -> Result<(), Error> {
        self.api
            .request(
                Method::DELETE,
                &format!("{}/resource_instances/{organization_id}", self.facts),
                None::<()>,
                None,
            )
            .await
    }

    pub async fn get_organizations(&self, user_id: &str) -> Result<(), Error> {
        self.api
            .get(
                &format!(
                    "{}/role_assignments?user={user_id}&resource=Organization",
                    self.facts
                ),
                None,
            )
            .await
    }

    pub async fn is_organization_admin(
        &self,
        user_id: &str,
        org_name: &str,
    ) -> Result<bool, Error> {
        let res: Vec<Value> = self
            .api
            .get(
                &format!(
                    "{}/role_assignments?user={user_id}&resource_instance=Organization:{org_name}",
                    self.facts
                ),
                None,
            )
            .await?;

        Ok(res[0].as_object().unwrap()["role"].as_str().unwrap() == "admin")
    }

    pub async fn create_organization_project(
        &self,
        org_name: &str,
        project_id: &str,
    ) -> Result<(), Error> {
        self.api
            .post(
                &format!("{}/relationship_tuples", self.facts),
                json!({
                    "subject": format!("Organization:{org_name}"),
                    "tenant": "default",
                    "relation": "parent",
                    "object": format!("Project:{project_id}"),
                }),
                None,
            )
            .await
    }

    pub async fn delete_organization_project(
        &self,
        org_name: &str,
        project_id: &str,
    ) -> Result<(), Error> {
        self.api
            .delete(
                &format!("{}/relationship_tuples", self.facts),
                json!({
                    "subject": format!("Organization:{org_name}"),
                    "relation": "parent",
                    "object": format!("Project:{project_id}"),
                }),
                None,
            )
            .await
    }

    pub async fn get_organization_projects(
        &self,
        org_name: &str,
    ) -> Result<Vec<OrganizationResource>, Error> {
        self.api
            .get(
                &format!(
                    "{}/relationship_tuples?subject=Organization:{org_name}&detailed=true",
                    self.facts
                ),
                None,
            )
            .await
    }

    pub async fn get_organization_members(&self, org_name: &str) -> Result<Vec<Value>, Error> {
        self.api
            .get(
                &format!(
                    "{}/role_assignments?resource_instance=Organization:{org_name}&role=member",
                    self.facts
                ),
                None,
            )
            .await
    }

    pub async fn create_organization_member(
        &self,
        org_name: &str,
        user_id: &str,
    ) -> Result<(), Error> {
        self.api
            .post(
                &format!("{}/role_assignments", self.facts),
                json!({
                    "role": "member",
                    "resource_instance": format!("Organization:{org_name}"),
                    "tenant": "default",
                    "user": user_id,
                }),
                None,
            )
            .await
    }

    pub async fn delete_organization_member(
        &self,
        org_name: &str,
        user_id: &str,
    ) -> Result<(), Error> {
        self.api
            .delete(
                &format!("{}/role_assignments", self.facts),
                json!({
                    "role": "member",
                    "resource_instance": format!("Organization:{org_name}"),
                    "tenant": "default",
                    "user": user_id,
                }),
                None,
            )
            .await
    }

    pub async fn get_user_projects(&self, user_id: &str) -> Result<Vec<ProjectPermissions>, Error> {
        let perms: HashMap<String, ProjectPermissions> = self
            .pdp
            .post(
                "/user-permissions",
                json!({
                    "user": {"key": user_id},
                    "resource_types": ["Project"],
                }),
                None,
            )
            .await?;

        Ok(perms.into_values().collect())
    }

    pub async fn allowed(
        &self,
        user_id: &str,
        project_id: &str,
        action: &str,
    ) -> Result<bool, Error> {
        let res: Value = self
            .pdp
            .post(
                "/allowed",
                json!({
                    "user": {"key": user_id},
                    "action": action,
                    "resource": {"type": "Project", "key": project_id, "tenant": "default"},
                }),
                None,
            )
            .await?;

        Ok(res["allow"].as_bool().unwrap())
    }
}

/// Struct to hold the following relationship tuple from permit
///
/// ```json
/// {
///   "subject": "Organization:London",
///   "relation": "parent",
///   "object": "Project:01HRAER7SMNPYZR3RYPAGHMFYW",
///   "id": "dfb57d795ba1432192a5b0ffd0293cae",
///   "tenant": "default",
///   "subject_id": "6eb3094331694b09ac1596fdb7834be5",
///   "relation_id": "cc1bf6e3e51e4b588c36a04552427461",
///   "object_id": "0af595f5ce834c7cad1cca513a1a6fd2",
///   "tenant_id": "4da8b268e96644609978dd62041b5fc6",
///   "organization_id": "5f504714eee841aaaef0d9546d2fd998",
///   "project_id": "b3492c78ccf44f7fb72615bdbfa58027",
///   "environment_id": "b3d12e0fd440433c8ba480bde8cb6cd2",
///   "created_at": "2024-03-07T15:27:59+00:00",
///   "updated_at": "2024-03-07T15:27:59+00:00",
///   "subject_details": {
///     "key": "London",
///     "tenant": "default",
///     "resource": "Organization",
///     "attributes": {}
///   },
///   "relation_details": {
///     "key": "parent",
///     "name": "parent",
///     "description": "Relation expresses possible 'parent' relation between subject of type 'Organization' to object of type 'Project'"
///   },
///   "object_details": {
///     "key": "01HRAER7SMNPYZR3RYPAGHMFYW",
///     "tenant": "default",
///     "resource": "Project",
///     "attributes": {}
///   },
///   "tenant_details": {
///     "key": "default",
///     "name": "Default Tenant",
///     "description": null,
///     "attributes": null
///   }
/// }
/// ```
#[derive(Debug, Serialize, Deserialize)]
pub struct OrganizationResource {
    pub subject: String,
    pub relation: String,
    pub object: String,
    pub id: String,

    /// The project which this organization is the parent of
    pub object_details: ObjectDetails,

    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

/// Struct to hold the following
/// ```json
/// {
///   "key": "01HRAER7SMNPYZR3RYPAGHMFYW",
///   "tenant": "default",
///   "resource": "Project",
///   "attributes": {}
/// }
/// ```
#[derive(Debug, Serialize, Deserialize)]
pub struct ObjectDetails {
    pub key: String,
    #[serde(default)]
    pub name: String,
    pub tenant: String,
    pub resource: String,
    pub attributes: Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProjectPermissions {
    pub resource: SimpleResource,
    pub permissions: Vec<String>,
    pub roles: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SimpleResource {
    pub key: String,
    pub r#type: String,
    pub attributes: Value,
}
