use std::collections::HashMap;

use async_trait::async_trait;
use http::{Method, Uri};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{json, Value};

use crate::claims::AccountTier;

use super::{Error, ServicesApiClient};

#[async_trait]
pub trait PermissionsDal {
    /// Get a user with the given ID
    async fn get_user(&self, user_id: &str) -> Result<User, Error>;

    /// Delete a user with the given ID
    async fn delete_user(&self, user_id: &str) -> Result<(), Error>;

    /// Create a new user and set their tier correctly
    async fn new_user(&self, user_id: &str) -> Result<User, Error>;

    /// Set a user to be a Pro user
    async fn make_pro(&self, user_id: &str) -> Result<(), Error>;

    /// Set a user to be a Free user
    async fn make_free(&self, user_id: &str) -> Result<(), Error>;
}

/// Simple user
#[derive(Clone, Deserialize, Debug)]
pub struct User {
    pub id: String,
    pub key: String,
    #[serde(deserialize_with = "deserialize_role")]
    pub roles: Vec<AccountTier>,
}

#[derive(Deserialize)]
struct RoleObject {
    role: AccountTier,
}

// Used to convert a Permit role into our internal `AccountTier` enum
fn deserialize_role<'de, D>(deserializer: D) -> Result<Vec<AccountTier>, D::Error>
where
    D: Deserializer<'de>,
{
    let roles = Vec::<RoleObject>::deserialize(deserializer)?;

    let mut role_vec: Vec<AccountTier> = roles.into_iter().map(|r| r.role).collect();

    role_vec.sort();

    Ok(role_vec)
}

#[derive(Clone)]
pub struct Client {
    /// The Permit.io API
    api: ServicesApiClient,
    /// The local Permit PDP (Policy decision point) API
    pdp: ServicesApiClient,
    /// The base URL path for 'facts' endpoints. Helps with building full URLs.
    facts: String,
}

impl Client {
    pub fn new(api_uri: Uri, pdp_uri: Uri, proj_id: String, env_id: String, api_key: &str) -> Self {
        Self {
            api: ServicesApiClient::new_with_bearer(api_uri, api_key),
            pdp: ServicesApiClient::new(pdp_uri),
            facts: format!("v2/facts/{}/{}", proj_id, env_id),
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

    async fn create_user(&self, user_id: &str) -> Result<User, Error> {
        self.api
            .post(
                &format!("{}/users", self.facts),
                json!({"key": user_id}),
                None,
            )
            .await
    }

    async fn assign_role(&self, user_id: &str, role: &AccountTier) -> Result<(), Error> {
        self.api
            .request_raw(
                Method::POST,
                &format!("{}/users/{user_id}/roles", self.facts),
                Some(json!({
                    "role": role,
                    "tenant": "default",
                })),
                None,
            )
            .await?;

        Ok(())
    }

    async fn unassign_role(&self, user_id: &str, role: &AccountTier) -> Result<(), Error> {
        self.api
            .request_raw(
                Method::DELETE,
                &format!("{}/users/{user_id}/roles", self.facts),
                Some(json!({
                    "role": role,
                    "tenant": "default",
                })),
                None,
            )
            .await?;

        Ok(())
    }
}

#[async_trait]
impl PermissionsDal for Client {
    async fn get_user(&self, user_id: &str) -> Result<User, Error> {
        self.api
            .get(&format!("{}/users/{user_id}", self.facts), None)
            .await
    }

    async fn delete_user(&self, user_id: &str) -> Result<(), Error> {
        self.api
            .request_raw(
                Method::DELETE,
                &format!("{}/users/{user_id}", self.facts),
                None::<()>,
                None,
            )
            .await?;

        Ok(())
    }

    async fn new_user(&self, user_id: &str) -> Result<User, Error> {
        let user = self.create_user(user_id).await?;
        self.make_free(&user.id).await?;

        self.get_user(&user.id).await
    }

    async fn make_pro(&self, user_id: &str) -> Result<(), Error> {
        let user = self.get_user(user_id).await?;

        if user.roles.contains(&AccountTier::Basic) {
            self.unassign_role(user_id, &AccountTier::Basic).await?;
        }

        self.assign_role(user_id, &AccountTier::Pro).await
    }

    async fn make_free(&self, user_id: &str) -> Result<(), Error> {
        let user = self.get_user(user_id).await?;

        if user.roles.contains(&AccountTier::Pro) {
            self.unassign_role(user_id, &AccountTier::Pro).await?;
        }

        self.assign_role(user_id, &AccountTier::Basic).await
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

#[cfg(test)]
mod tests {
    use std::env;

    use http::StatusCode;
    use serde_json::Value;
    use serial_test::serial;
    use test_context::{test_context, AsyncTestContext};

    use crate::{backends::client::Error, claims::AccountTier};

    use super::*;

    impl Client {
        async fn clear_users(&self) {
            let users: Value = self
                .api
                .get(&format!("{}/users", self.facts), None)
                .await
                .unwrap();

            for user in users["data"].as_array().unwrap() {
                let user_id = user["id"].as_str().unwrap();
                self.delete_user(user_id).await.unwrap();
            }
        }
    }

    impl AsyncTestContext for Client {
        async fn setup() -> Self {
            let api_key = env::var("PERMIT_API_KEY").expect("PERMIT_API_KEY to be set. You can copy the testing API key from the Testing environment on Permit.io.");
            let client = Client::new(
                "https://api.eu-central-1.permit.io".parse().unwrap(),
                "http://localhost:7000".parse().unwrap(),
                "default".to_string(),
                "testing".to_string(),
                &api_key,
            );

            client.clear_users().await;

            client
        }

        async fn teardown(self) {
            self.clear_users().await;
        }
    }

    #[test_context(Client)]
    #[tokio::test]
    #[serial]
    async fn test_user_flow(client: &mut Client) {
        let user = client.create_user("test_user").await.unwrap();
        let user_actual = client.get_user("test_user").await.unwrap();

        assert_eq!(user.id, user_actual.id);

        // Can also get user by permit id
        client.get_user(&user.id).await.unwrap();

        // Now delete the user
        client.delete_user("test_user").await.unwrap();
        let res = client.get_user("test_user").await;

        assert!(matches!(
            res,
            Err(Error::RequestError(StatusCode::NOT_FOUND))
        ));
    }

    #[test_context(Client)]
    #[tokio::test]
    #[serial]
    async fn test_tiers_flow(client: &mut Client) {
        let user = client.create_user("tier_user").await.unwrap();

        assert!(user.roles.is_empty());

        // Make user a pro
        client
            .assign_role("tier_user", &AccountTier::Pro)
            .await
            .unwrap();
        let user = client.get_user("tier_user").await.unwrap();

        assert_eq!(user.roles, vec![AccountTier::Pro]);

        // Make user a free user
        client
            .assign_role("tier_user", &AccountTier::Basic)
            .await
            .unwrap();
        let user = client.get_user("tier_user").await.unwrap();

        assert_eq!(user.roles, vec![AccountTier::Basic, AccountTier::Pro]);

        // Remove the pro role
        client
            .unassign_role("tier_user", &AccountTier::Pro)
            .await
            .unwrap();
        let user = client.get_user("tier_user").await.unwrap();

        assert_eq!(user.roles, vec![AccountTier::Basic]);
    }

    #[test_context(Client)]
    #[tokio::test]
    #[serial]
    async fn test_user_complex_flow(client: &mut Client) {
        let user = client.new_user("jane").await.unwrap();
        assert_eq!(
            user.roles,
            vec![AccountTier::Basic],
            "making a new user should default to Free tier"
        );

        client.make_pro("jane").await.unwrap();
        client.make_pro("jane").await.unwrap();

        let user = client.get_user("jane").await.unwrap();
        assert_eq!(
            user.roles,
            vec![AccountTier::Pro],
            "changing to Pro should remove Free"
        );
    }
}
