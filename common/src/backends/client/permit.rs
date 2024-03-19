use http::Method;
use serde::{Deserialize, Deserializer};
use serde_json::json;

use crate::claims::AccountTier;

use super::{Error, ServicesApiClient};

#[allow(async_fn_in_trait)]
pub trait PermissionsDal {
    /// Low-level function to create a new user. Should almost always use [Self::new_user()] instead
    async fn create_user(&self, user_id: &str) -> Result<User, Error>;

    /// Get a user with the given ID
    async fn get_user(&self, user_id: &str) -> Result<User, Error>;

    /// Delete a user with the given ID
    async fn delete_user(&self, user_id: &str) -> Result<(), Error>;

    /// Low-level function to assign a specific role to a user. Should almost always use [Self::make_pro()] or [Self::make_free()] instead
    async fn assign_role(&self, user_id: &str, role: &AccountTier) -> Result<(), Error>;

    /// Low-level function to remove a specific role from a user. Should almost always use [Self::make_pro()] or [Self::make_free()] instead
    async fn unassign_role(&self, user_id: &str, role: &AccountTier) -> Result<(), Error>;

    /// Create a new user and set their tier correctly
    async fn new_user(&self, user_id: &str) -> Result<User, Error> {
        let user = self.create_user(user_id).await?;
        self.make_free(&user.id).await?;

        self.get_user(&user.id).await
    }

    /// Set a user to be a Pro user
    async fn make_pro(&self, user_id: &str) -> Result<(), Error> {
        let user = self.get_user(user_id).await?;

        if user.roles.contains(&AccountTier::Basic) {
            self.unassign_role(user_id, &AccountTier::Basic).await?;
        }

        self.assign_role(user_id, &AccountTier::Pro).await
    }

    /// Set a user to be a Free user
    async fn make_free(&self, user_id: &str) -> Result<(), Error> {
        let user = self.get_user(user_id).await?;

        if user.roles.contains(&AccountTier::Pro) {
            self.unassign_role(user_id, &AccountTier::Pro).await?;
        }

        self.assign_role(user_id, &AccountTier::Basic).await
    }
}

/// Simple user
#[derive(Deserialize, Debug)]
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
    client: ServicesApiClient,
    environment: String,
}

impl Client {
    /// Create a new client with the given API key and targeting the given environment
    pub fn new(api_key: &str, environment: &str) -> Self {
        Self {
            client: ServicesApiClient::new_with_bearer(
                "https://api.eu-central-1.permit.io".parse().unwrap(),
                api_key,
            ),
            environment: environment.to_string(),
        }
    }
}

impl PermissionsDal for Client {
    async fn create_user(&self, user_id: &str) -> Result<User, Error> {
        let Self { environment, .. } = self;

        self.client
            .post(
                &format!("v2/facts/default/{environment}/users"),
                json!({"key": user_id}),
                None,
            )
            .await
    }

    async fn get_user(&self, user_id: &str) -> Result<User, Error> {
        let Self { environment, .. } = self;

        self.client
            .get(
                &format!("v2/facts/default/{environment}/users/{user_id}"),
                None,
            )
            .await
    }

    async fn delete_user(&self, user_id: &str) -> Result<(), Error> {
        let Self { environment, .. } = self;

        self.client
            .request_raw(
                Method::DELETE,
                &format!("v2/facts/default/{environment}/users/{user_id}"),
                None::<()>,
                None,
            )
            .await?;

        Ok(())
    }

    async fn assign_role(&self, user_id: &str, role: &AccountTier) -> Result<(), Error> {
        let Self { environment, .. } = self;

        self.client
            .request_raw(
                Method::POST,
                &format!("v2/facts/default/{environment}/users/{user_id}/roles"),
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
        let Self { environment, .. } = self;

        self.client
            .request_raw(
                Method::DELETE,
                &format!("v2/facts/default/{environment}/users/{user_id}/roles"),
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

#[cfg(test)]
mod tests {
    use async_once_cell::OnceCell;
    use http::StatusCode;
    use serde_json::Value;
    use test_context::{test_context, AsyncTestContext};

    use crate::{backends::client::Error, claims::AccountTier};

    use super::*;

    impl Client {
        async fn clear_users(&self) {
            let Self { environment, .. } = self;

            let users: Value = self
                .client
                .get(&format!("v2/facts/default/{environment}/users"), None)
                .await
                .unwrap();

            for user in users["data"].as_array().unwrap() {
                let user_id = user["id"].as_str().unwrap();
                self.delete_user(user_id).await.unwrap();
            }
        }
    }

    // Used to ensure that the cleanup steps are only run once while blocking other threads
    static CLEANUP_CALLED: OnceCell<()> = OnceCell::new();

    impl AsyncTestContext for Client {
        async fn setup() -> Self {
            let api_key = env!("PERMIT_API_KEY");
            let client = Client::new(api_key, "testing");

            CLEANUP_CALLED
                .get_or_init(async {
                    client.clear_users().await;
                })
                .await;

            client
        }
    }

    #[test_context(Client)]
    #[tokio::test]
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
