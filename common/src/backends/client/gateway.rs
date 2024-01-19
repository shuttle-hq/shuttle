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

#[async_trait::async_trait]
trait ProjectsDal {
    /// Get the projects that belong to a user
    async fn get_user_projects(
        &self,
        user_token: &str,
    ) -> Result<Vec<models::project::Response>, Error>;
}

#[async_trait::async_trait]
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
    use shuttle_common_tests::gateway::mocked_gateway_server;

    use crate::models::project::{Response, State};

    use super::{GatewayClient, ProjectsDal};

    #[tokio::test]
    async fn get_user_projects() {
        let server = mocked_gateway_server().await;

        let client =
            GatewayClient::new(server.uri().parse().unwrap(), server.uri().parse().unwrap());

        let res = client.get_user_projects("t").await.unwrap();

        assert_eq!(
            res,
            vec![Response {
                id: "01HMGS32BRKBFSY82WZE2WZZRY".to_string(),
                name: "mock-project-1".to_string(),
                state: State::Stopped,
                idle_minutes: Some(30)
            }]
        )
    }
}