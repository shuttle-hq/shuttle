use headers::Authorization;
use http::{Method, Uri};

use crate::models;

use super::{Error, ServicesApiClient};

/// Wrapper struct to make API calls to gateway easier
#[derive(Clone)]
pub struct GatewayClient {
    public_client: ServicesApiClient,
    private_client: ServicesApiClient,
}

impl GatewayClient {
    /// Make a gateway client that is able to call the public and private APIs on gateway
    pub fn new(public_uri: Uri, private_uri: Uri) -> Self {
        Self {
            public_client: ServicesApiClient::new(public_uri),
            private_client: ServicesApiClient::new(private_uri),
        }
    }

    /// Get the client of public API calls
    pub fn public_client(&self) -> &ServicesApiClient {
        &self.public_client
    }

    /// Get the client of private API calls
    pub fn private_client(&self) -> &ServicesApiClient {
        &self.private_client
    }
}

/// Interact with all the data relating to projects
trait ProjectsDal {
    /// Get the projects that belong to a user
    async fn get_user_projects(
        &self,
        user_token: &str,
    ) -> Result<Vec<models::project::Response>, Error>;

    /// Get the IDs of all the projects belonging to a user
    async fn get_user_project_ids(&self, user_token: &str) -> Result<Vec<String>, Error> {
        let ids = self
            .get_user_projects(user_token)
            .await?
            .into_iter()
            .map(|p| p.id)
            .collect();

        Ok(ids)
    }
}

impl ProjectsDal for GatewayClient {
    async fn get_user_projects(
        &self,
        user_token: &str,
    ) -> Result<Vec<models::project::Response>, Error> {
        let projects = self
            .public_client
            .request(
                Method::GET,
                "projects",
                None::<()>,
                Some(Authorization::bearer(user_token).expect("to build an authorization bearer")),
            )
            .await?;

        Ok(projects)
    }
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use test_context::{test_context, AsyncTestContext};

    use crate::models::project::{Response, State};
    use crate::test_utils::mocked_gateway_server;

    use super::{GatewayClient, ProjectsDal};

    #[async_trait]
    impl AsyncTestContext for GatewayClient {
        async fn setup() -> Self {
            let server = mocked_gateway_server().await;

            GatewayClient::new(server.uri().parse().unwrap(), server.uri().parse().unwrap())
        }

        async fn teardown(mut self) {}
    }

    #[test_context(GatewayClient)]
    #[tokio::test]
    async fn get_user_projects(client: &mut GatewayClient) {
        let res = client.get_user_projects("user-1").await.unwrap();

        assert_eq!(
            res,
            vec![
                Response {
                    id: "id1".to_string(),
                    name: "user-1-project-1".to_string(),
                    state: State::Stopped,
                    idle_minutes: Some(30)
                },
                Response {
                    id: "id2".to_string(),
                    name: "user-1-project-2".to_string(),
                    state: State::Ready,
                    idle_minutes: Some(30)
                }
            ]
        )
    }

    #[test_context(GatewayClient)]
    #[tokio::test]
    async fn get_user_project_ids(client: &mut GatewayClient) {
        let res = client.get_user_project_ids("user-2").await.unwrap();

        assert_eq!(res, vec!["id3"])
    }
}
