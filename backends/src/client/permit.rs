use anyhow::Error;
use async_trait::async_trait;
use permit_client_rs::{
    apis::{
        resource_instances_api::{create_resource_instance, delete_resource_instance},
        role_assignments_api::{assign_role, unassign_role},
        users_api::{create_user, delete_user, get_user},
    },
    models::{
        ResourceInstanceCreate, RoleAssignmentCreate, RoleAssignmentRemove, UserCreate, UserRead,
    },
};
use permit_pdp_client_rs::{
    apis::authorization_api_api::{
        get_user_permissions_user_permissions_post, is_allowed_allowed_post,
    },
    models::{AuthorizationQuery, Resource, User, UserPermissionsQuery, UserPermissionsResult},
};
use shuttle_common::claims::AccountTier;

#[async_trait]
pub trait PermissionsDal {
    /// Get a user with the given ID
    async fn get_user(&self, user_id: &str) -> Result<UserRead, Error>;
    /// Delete a user with the given ID
    async fn delete_user(&self, user_id: &str) -> Result<(), Error>;
    /// Create a new user and set their tier correctly
    async fn new_user(&self, user_id: &str) -> Result<UserRead, Error>;
    /// Set a user to be a Pro user
    async fn make_pro(&self, user_id: &str) -> Result<(), Error>;
    /// Set a user to be a Basic user
    async fn make_basic(&self, user_id: &str) -> Result<(), Error>;

    /// Creates a Project resource and assigns the user as admin for that project
    async fn create_project(&self, user_id: &str, project_id: &str) -> Result<(), Error>;
    /// Deletes a Project resource
    async fn delete_project(&self, project_id: &str) -> Result<(), Error>;
}

#[derive(Clone)]
pub struct Client {
    /// The Permit.io API
    api: permit_client_rs::apis::configuration::Configuration,
    /// The local Permit PDP (Policy decision point) API
    pdp: permit_pdp_client_rs::apis::configuration::Configuration,
    proj_id: String,
    env_id: String,
}

impl Client {
    pub fn new(
        api_uri: String,
        pdp_uri: String,
        proj_id: String,
        env_id: String,
        api_key: String,
    ) -> Self {
        Self {
            api: permit_client_rs::apis::configuration::Configuration {
                base_path: api_uri
                    .strip_suffix('/')
                    .map(ToOwned::to_owned)
                    .unwrap_or(api_uri),
                user_agent: None,
                bearer_access_token: Some(api_key.clone()),
                ..Default::default()
            },
            pdp: permit_pdp_client_rs::apis::configuration::Configuration {
                base_path: pdp_uri
                    .strip_suffix('/')
                    .map(ToOwned::to_owned)
                    .unwrap_or(pdp_uri),
                user_agent: None,
                bearer_access_token: Some(api_key),
                ..Default::default()
            },
            proj_id,
            env_id,
        }
    }

    // /// Assigns a user to an org directly without creating the org first
    // pub async fn create_organization(&self, user_id: &str, org_name: &str) -> Result<(), Error> {
    //     self.api
    //         .post(
    //             &format!("{}/resource_instances", self.facts),
    //             json!({
    //                 "key": org_name,
    //                 "tenant": "default",
    //                 "resource": "Organization",
    //             }),
    //             None,
    //         )
    //         .await?;

    //     self.api
    //         .post(
    //             &format!("{}/role_assignments", self.facts),
    //             json!({
    //                 "role": "admin",
    //                 "resource_instance": format!("Organization:{org_name}"),
    //                 "tenant": "default",
    //                 "user": user_id,
    //             }),
    //             None,
    //         )
    //         .await
    // }

    // pub async fn delete_organization(&self, org_id: &str) -> Result<(), Error> {
    //     self.api
    //         .request(
    //             Method::DELETE,
    //             &format!("{}/resource_instances/{org_id}", self.facts),
    //             None::<()>,
    //             None,
    //         )
    //         .await
    // }

    // pub async fn get_organizations(&self, user_id: &str) -> Result<(), Error> {
    //     self.api
    //         .get(
    //             &format!(
    //                 "{}/role_assignments?user={user_id}&resource=Organization",
    //                 self.facts
    //             ),
    //             None,
    //         )
    //         .await
    // }

    // pub async fn is_organization_admin(
    //     &self,
    //     user_id: &str,
    //     org_name: &str,
    // ) -> Result<bool, Error> {
    //     let res: Vec<Value> = self
    //         .api
    //         .get(
    //             &format!(
    //                 "{}/role_assignments?user={user_id}&resource_instance=Organization:{org_name}",
    //                 self.facts
    //             ),
    //             None,
    //         )
    //         .await?;

    //     Ok(res[0].as_object().unwrap()["role"].as_str().unwrap() == "admin")
    // }

    // pub async fn create_organization_project(
    //     &self,
    //     org_name: &str,
    //     project_id: &str,
    // ) -> Result<(), Error> {
    //     self.api
    //         .post(
    //             &format!("{}/relationship_tuples", self.facts),
    //             json!({
    //                 "subject": format!("Organization:{org_name}"),
    //                 "tenant": "default",
    //                 "relation": "parent",
    //                 "object": format!("Project:{project_id}"),
    //             }),
    //             None,
    //         )
    //         .await
    // }

    // pub async fn delete_organization_project(
    //     &self,
    //     org_name: &str,
    //     project_id: &str,
    // ) -> Result<(), Error> {
    //     self.api
    //         .delete(
    //             &format!("{}/relationship_tuples", self.facts),
    //             json!({
    //                 "subject": format!("Organization:{org_name}"),
    //                 "relation": "parent",
    //                 "object": format!("Project:{project_id}"),
    //             }),
    //             None,
    //         )
    //         .await
    // }

    // pub async fn get_organization_projects(
    //     &self,
    //     org_name: &str,
    // ) -> Result<Vec<OrganizationResource>, Error> {
    //     self.api
    //         .get(
    //             &format!(
    //                 "{}/relationship_tuples?subject=Organization:{org_name}&detailed=true",
    //                 self.facts
    //             ),
    //             None,
    //         )
    //         .await
    // }

    // pub async fn get_organization_members(&self, org_name: &str) -> Result<Vec<Value>, Error> {
    //     self.api
    //         .get(
    //             &format!(
    //                 "{}/role_assignments?resource_instance=Organization:{org_name}&role=member",
    //                 self.facts
    //             ),
    //             None,
    //         )
    //         .await
    // }

    // pub async fn create_organization_member(
    //     &self,
    //     org_name: &str,
    //     user_id: &str,
    // ) -> Result<(), Error> {
    //     self.api
    //         .post(
    //             &format!("{}/role_assignments", self.facts),
    //             json!({
    //                 "role": "member",
    //                 "resource_instance": format!("Organization:{org_name}"),
    //                 "tenant": "default",
    //                 "user": user_id,
    //             }),
    //             None,
    //         )
    //         .await
    // }

    // pub async fn delete_organization_member(
    //     &self,
    //     org_name: &str,
    //     user_id: &str,
    // ) -> Result<(), Error> {
    //     self.api
    //         .delete(
    //             &format!("{}/role_assignments", self.facts),
    //             json!({
    //                 "role": "member",
    //                 "resource_instance": format!("Organization:{org_name}"),
    //                 "tenant": "default",
    //                 "user": user_id,
    //             }),
    //             None,
    //         )
    //         .await
    // }

    pub async fn get_user_projects(
        &self,
        user_id: &str,
    ) -> Result<Vec<UserPermissionsResult>, Error> {
        let perms = get_user_permissions_user_permissions_post(
            &self.pdp,
            UserPermissionsQuery {
                user: Box::new(User {
                    key: user_id.to_owned(),
                    ..Default::default()
                }),
                resource_types: Some(vec!["Project".to_owned()]),
                tenants: Some(vec!["default".to_owned()]),
                ..Default::default()
            },
            None,
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
        // NOTE: This API function was modified in upstream to use AuthorizationQuery
        let res = is_allowed_allowed_post(
            &self.pdp,
            AuthorizationQuery {
                user: Box::new(User {
                    key: user_id.to_owned(),
                    ..Default::default()
                }),
                action: action.to_owned(),
                resource: Box::new(Resource {
                    r#type: "Project".to_string(),
                    key: Some(project_id.to_owned()),
                    tenant: Some("default".to_owned()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            None,
            None,
        )
        .await?;

        Ok(res.allow.unwrap_or_default())
    }

    async fn create_user(&self, user_id: &str) -> Result<UserRead, Error> {
        Ok(create_user(
            &self.api,
            &self.proj_id,
            &self.env_id,
            UserCreate {
                key: user_id.to_owned(),
                ..Default::default()
            },
        )
        .await?)
    }

    async fn assign_role(&self, user_id: &str, role: &AccountTier) -> Result<(), Error> {
        assign_role(
            &self.api,
            &self.proj_id,
            &self.env_id,
            RoleAssignmentCreate {
                role: role.to_string(),
                tenant: Some("default".to_owned()),
                resource_instance: None,
                user: user_id.to_owned(),
            },
        )
        .await?;

        Ok(())
    }

    async fn unassign_role(&self, user_id: &str, role: &AccountTier) -> Result<(), Error> {
        unassign_role(
            &self.api,
            &self.proj_id,
            &self.env_id,
            RoleAssignmentRemove {
                role: role.to_string(),
                tenant: Some("default".to_owned()),
                resource_instance: None,
                user: user_id.to_owned(),
            },
        )
        .await?;

        Ok(())
    }
}

#[async_trait]
impl PermissionsDal for Client {
    async fn get_user(&self, user_id: &str) -> Result<UserRead, Error> {
        Ok(get_user(&self.api, &self.proj_id, &self.env_id, user_id).await?)
    }

    async fn delete_user(&self, user_id: &str) -> Result<(), Error> {
        Ok(delete_user(&self.api, &self.proj_id, &self.env_id, user_id).await?)
    }

    async fn new_user(&self, user_id: &str) -> Result<UserRead, Error> {
        let user = self.create_user(user_id).await?;
        self.make_basic(&user.id.to_string()).await?;

        self.get_user(&user.id.to_string()).await
    }

    async fn make_pro(&self, user_id: &str) -> Result<(), Error> {
        let user = self.get_user(user_id).await?;

        if user.roles.is_some_and(|roles| {
            roles
                .iter()
                .any(|r| r.role == AccountTier::Basic.to_string())
        }) {
            self.unassign_role(user_id, &AccountTier::Basic).await?;
        }

        self.assign_role(user_id, &AccountTier::Pro).await
    }

    async fn make_basic(&self, user_id: &str) -> Result<(), Error> {
        let user = self.get_user(user_id).await?;

        if user
            .roles
            .is_some_and(|roles| roles.iter().any(|r| r.role == AccountTier::Pro.to_string()))
        {
            self.unassign_role(user_id, &AccountTier::Pro).await?;
        }

        self.assign_role(user_id, &AccountTier::Basic).await
    }

    async fn create_project(&self, user_id: &str, project_id: &str) -> Result<(), Error> {
        create_resource_instance(
            &self.api,
            &self.proj_id,
            &self.env_id,
            ResourceInstanceCreate {
                key: project_id.to_owned(),
                tenant: "default".to_owned(),
                resource: "Project".to_owned(),
                attributes: None,
            },
        )
        .await?;

        assign_role(
            &self.api,
            &self.proj_id,
            &self.env_id,
            RoleAssignmentCreate {
                role: "admin".to_owned(),
                tenant: Some("default".to_owned()),
                resource_instance: Some(format!("Project:{project_id}")),
                user: user_id.to_owned(),
            },
        )
        .await?;

        Ok(())
    }

    async fn delete_project(&self, project_id: &str) -> Result<(), Error> {
        Ok(delete_resource_instance(
            &self.api,
            &self.proj_id,
            &self.env_id,
            format!("Project:{project_id}").as_str(),
        )
        .await?)
    }
}

#[cfg(test)]
mod tests {
    use std::env;

    use http::StatusCode;
    use permit_client_rs::apis::users_api::list_users;
    use serial_test::serial;
    use test_context::{test_context, AsyncTestContext};

    use crate::client::Error;

    use super::*;

    impl Client {
        async fn clear_users(&self) {
            let users = list_users(
                &self.api,
                &self.proj_id,
                &self.env_id,
                None,
                None,
                None,
                None,
                Some(100),
            )
            .await
            .unwrap();

            for user in users.data {
                self.delete_user(&user.id.to_string()).await.unwrap();
            }
        }
    }

    impl AsyncTestContext for Client {
        async fn setup() -> Self {
            let api_key = env::var("PERMIT_API_KEY").expect("PERMIT_API_KEY to be set. You can copy the testing API key from the Testing environment on Permit.io.");
            let client = Client::new(
                "https://api.eu-central-1.permit.io".to_owned(),
                "http://localhost:7000".to_owned(),
                "default".to_owned(),
                "testing".to_owned(),
                api_key,
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
        client.get_user(&user.id.to_string()).await.unwrap();

        // Now delete the user
        client.delete_user("test_user").await.unwrap();
        let res = client.get_user("test_user").await;

        assert!(matches!(
            res.unwrap_err().downcast_ref::<Error>().unwrap(),
            Error::RequestError(StatusCode::NOT_FOUND)
        ));
    }

    #[test_context(Client)]
    #[tokio::test]
    #[serial]
    async fn test_tiers_flow(client: &mut Client) {
        let user = client.create_user("tier_user").await.unwrap();

        assert!(user.roles.unwrap().is_empty());

        // Make user a pro
        client
            .assign_role("tier_user", &AccountTier::Pro)
            .await
            .unwrap();
        let user = client.get_user("tier_user").await.unwrap();

        assert_eq!(user.roles.as_ref().unwrap().len(), 1);
        assert_eq!(
            user.roles.as_ref().unwrap()[0].role,
            AccountTier::Pro.to_string()
        );

        // Make user a free user
        client
            .assign_role("tier_user", &AccountTier::Basic)
            .await
            .unwrap();
        let user = client.get_user("tier_user").await.unwrap();

        assert_eq!(user.roles.as_ref().unwrap().len(), 2);
        assert_eq!(
            user.roles.as_ref().unwrap()[0].role,
            AccountTier::Basic.to_string()
        );
        assert_eq!(
            user.roles.as_ref().unwrap()[1].role,
            AccountTier::Pro.to_string()
        );

        // Remove the pro role
        client
            .unassign_role("tier_user", &AccountTier::Pro)
            .await
            .unwrap();
        let user = client.get_user("tier_user").await.unwrap();

        assert_eq!(user.roles.as_ref().unwrap().len(), 1);
        assert_eq!(
            user.roles.as_ref().unwrap()[0].role,
            AccountTier::Basic.to_string()
        );
    }

    #[test_context(Client)]
    #[tokio::test]
    #[serial]
    async fn test_user_complex_flow(client: &mut Client) {
        let user = client.new_user("jane").await.unwrap();
        assert_eq!(user.roles.as_ref().unwrap().len(), 1);
        assert_eq!(
            user.roles.as_ref().unwrap()[0].role,
            AccountTier::Basic.to_string(),
            "making a new user should default to Free tier"
        );

        client.make_pro("jane").await.unwrap();

        let user = client.get_user("jane").await.unwrap();
        assert_eq!(user.roles.as_ref().unwrap().len(), 1);
        assert_eq!(
            user.roles.as_ref().unwrap()[0].role,
            AccountTier::Pro.to_string(),
            "changing to Pro should remove Free"
        );
    }
}
