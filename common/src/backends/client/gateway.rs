use headers::Authorization;
use http::{Method, Uri};

use crate::models;

use super::{Error, ServicesApiClient};

#[derive(Clone)]
pub struct GatewayClient {
    public_client: ServicesApiClient,
    private_client: ServicesApiClient,
}

impl GatewayClient {
    pub fn new(public_uri: Uri, private_uri: Uri) -> Self {
        Self {
            public_client: ServicesApiClient::new(public_uri),
            private_client: ServicesApiClient::new(private_uri),
        }
    }

    pub fn public_client(&self) -> &ServicesApiClient {
        &self.public_client
    }

    pub fn private_client(&self) -> &ServicesApiClient {
        &self.private_client
    }

    /// Get the projects that belong to a user
    pub async fn get_user_projects(
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
    #[test]
    fn failing_test() {
        assert!(false);
    }
}
