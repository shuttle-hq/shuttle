use http::Method;
use tracing::instrument;

use crate::models;

use super::{header_map_with_bearer, Error, ServicesApiClient};

/// Interact with all the data relating to projects
#[allow(async_fn_in_trait)]
pub trait ProjectsDal {
    /// Get a user project
    async fn get_user_project(
        &self,
        user_token: &str,
        project_name: &str,
    ) -> Result<models::project::Response, Error>;

    /// Check the HEAD of a user project
    async fn head_user_project(&self, user_token: &str, project_name: &str) -> Result<bool, Error>;

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

impl ProjectsDal for ServicesApiClient {
    #[instrument(skip_all)]
    async fn get_user_project(
        &self,
        user_token: &str,
        project_name: &str,
    ) -> Result<models::project::Response, Error> {
        self.get(
            format!("projects/{}", project_name).as_str(),
            Some(header_map_with_bearer(user_token)),
        )
        .await
    }

    #[instrument(skip_all)]
    async fn head_user_project(&self, user_token: &str, project_name: &str) -> Result<bool, Error> {
        self.request_raw(
            Method::HEAD,
            format!("projects/{}", project_name).as_str(),
            None::<()>,
            Some(header_map_with_bearer(user_token)),
        )
        .await?;

        Ok(true)
    }

    #[instrument(skip_all)]
    async fn get_user_projects(
        &self,
        user_token: &str,
    ) -> Result<Vec<models::project::Response>, Error> {
        self.get("projects", Some(header_map_with_bearer(user_token)))
            .await
    }
}

#[cfg(test)]
mod tests {
    use test_context::{test_context, AsyncTestContext};

    use crate::backends::client::ServicesApiClient;
    use crate::models::project::{Response, State};
    use crate::test_utils::get_mocked_gateway_server;

    use super::ProjectsDal;

    impl AsyncTestContext for ServicesApiClient {
        async fn setup() -> Self {
            let server = get_mocked_gateway_server().await;

            ServicesApiClient::new(server.uri().parse().unwrap())
        }

        async fn teardown(self) {}
    }

    #[test_context(ServicesApiClient)]
    #[tokio::test]
    async fn get_user_projects(client: &mut ServicesApiClient) {
        let res = client.get_user_projects("user-1").await.unwrap();

        assert_eq!(
            res,
            vec![
                Response {
                    id: "00000000000000000000000001".to_string(),
                    name: "user-1-project-1".to_string(),
                    state: State::Stopped,
                    idle_minutes: Some(30)
                },
                Response {
                    id: "00000000000000000000000002".to_string(),
                    name: "user-1-project-2".to_string(),
                    state: State::Ready,
                    idle_minutes: Some(30)
                }
            ]
        )
    }

    #[test_context(ServicesApiClient)]
    #[tokio::test]
    async fn get_user_project_ids(client: &mut ServicesApiClient) {
        let res = client.get_user_project_ids("user-2").await.unwrap();

        assert_eq!(res, vec!["00000000000000000000000003"])
    }
}
