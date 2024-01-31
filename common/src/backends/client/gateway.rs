use headers::Authorization;
use http::{Method, Uri};
use tracing::instrument;

use crate::models;

use super::{Error, ServicesApiClient};
#[cfg(feature = "persist")]
use crate::persistence::{
    deployment::{
        ActiveDeploymentsGetter, AddressGetter, Deployment, DeploymentRunnable, DeploymentUpdater,
    },
    service::Service,
    state::DeploymentState,
    DeployerPersistenceApi,
};
#[cfg(feature = "persist")]
use std::net::SocketAddr;
#[cfg(feature = "persist")]
use ulid::Ulid;
#[cfg(feature = "persist")]
use uuid::Uuid;

/// Wrapper struct to make API calls to gateway easier
#[derive(Clone)]
pub struct Client {
    public_client: ServicesApiClient,
    private_client: ServicesApiClient,
}

impl Client {
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
#[allow(async_fn_in_trait)]
pub trait ProjectsDal {
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

#[cfg(feature = "persist")]
#[async_trait::async_trait]
impl DeploymentUpdater for Client {
    type Err = Error;

    async fn set_address(&self, _id: &Uuid, _address: &SocketAddr) -> Result<(), Error> {
        todo!()
    }

    async fn set_is_next(&self, _id: &Uuid, _is_next: bool) -> Result<(), Error> {
        todo!()
    }

    async fn set_state(&self, _state: DeploymentState) -> Result<(), Error> {
        todo!()
    }

    async fn update_deployment(&self, _state: DeploymentState) -> Result<(), Error> {
        todo!()
    }
}

#[cfg(feature = "persist")]
#[async_trait::async_trait]
impl ActiveDeploymentsGetter for Client {
    type Err = Error;

    async fn get_active_deployments(
        &self,
        _service_id: &ulid::Ulid,
    ) -> std::result::Result<Vec<Uuid>, Error> {
        todo!()
    }
}

#[cfg(feature = "persist")]
#[async_trait::async_trait]
impl AddressGetter for Client {
    type Err = Error;

    async fn get_address_for_service(
        &self,
        _service_name: &str,
    ) -> std::result::Result<Option<std::net::SocketAddr>, Error> {
        todo!()
    }
}

#[cfg(feature = "persist")]
#[async_trait::async_trait]
impl DeployerPersistenceApi for Client {
    type MasterErr = Error;

    async fn insert_deployment(
        &self,
        _deployment: impl Into<&Deployment> + Send,
    ) -> Result<(), Error> {
        todo!()
    }

    async fn get_deployment(&self, _id: &Uuid) -> Result<Option<Deployment>, Error> {
        todo!()
    }

    async fn get_deployments(
        &self,
        _service_id: &Ulid,
        _offset: u32,
        _limit: u32,
    ) -> Result<Vec<Deployment>, Error> {
        todo!()
    }

    async fn get_active_deployment(&self, _service_id: &Ulid) -> Result<Option<Deployment>, Error> {
        todo!()
    }

    async fn cleanup_invalid_states(&self) -> Result<(), Error> {
        todo!()
    }

    async fn get_service_by_name(&self, _name: &str) -> Result<Option<Service>, Error> {
        todo!()
    }

    async fn get_or_create_service(&self, _name: &str) -> Result<Service, Error> {
        todo!()
    }

    async fn delete_service(&self, _id: &Ulid) -> Result<(), Error> {
        todo!()
    }
    async fn get_all_services(&self) -> Result<Vec<Service>, Error> {
        todo!()
    }

    async fn get_all_runnable_deployments(&self) -> Result<Vec<DeploymentRunnable>, Error> {
        todo!()
    }
    async fn get_runnable_deployment(
        &self,
        _id: &Uuid,
    ) -> Result<Option<DeploymentRunnable>, Error> {
        todo!()
    }
}

impl ProjectsDal for Client {
    #[instrument(skip_all)]
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
    use crate::test_utils::get_mocked_gateway_server;

    use super::{Client, ProjectsDal};

    #[async_trait]
    impl AsyncTestContext for Client {
        async fn setup() -> Self {
            let server = get_mocked_gateway_server().await;

            Client::new(server.uri().parse().unwrap(), server.uri().parse().unwrap())
        }

        async fn teardown(mut self) {}
    }

    #[test_context(Client)]
    #[tokio::test]
    async fn get_user_projects(client: &mut Client) {
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

    #[test_context(Client)]
    #[tokio::test]
    async fn get_user_project_ids(client: &mut Client) {
        let res = client.get_user_project_ids("user-2").await.unwrap();

        assert_eq!(res, vec!["id3"])
    }
}
