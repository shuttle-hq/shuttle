use bytes::Bytes;
use headers::{ContentType, Header, HeaderMapExt};
use http::{Method, Request, StatusCode, Uri};
use hyper::{
    body,
    client::{connect::Connect, HttpConnector},
    Body, Client,
};
use opentelemetry::global;
use opentelemetry_http::HeaderInjector;
use serde::{de::DeserializeOwned, Serialize};
use thiserror::Error;
use tracing::{trace, Span};
use tracing_opentelemetry::OpenTelemetrySpanExt;

pub mod gateway;
mod resource_recorder;

pub use gateway::ProjectsDal;
pub use resource_recorder::ResourceDal;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Hyper error: {0}")]
    Hyper(#[from] hyper::Error),
    #[error("Serde JSON error: {0}")]
    SerdeJson(#[from] serde_json::Error),
    #[error("Hyper error: {0}")]
    Http(#[from] hyper::http::Error),
    #[error("Request did not return correctly. Got status code: {0}")]
    RequestError(StatusCode),
    #[error("GRpc request did not return correctly. Got status code: {0}")]
    GrpcError(#[from] tonic::Status),
}

/// `Hyper` wrapper to make request to RESTful Shuttle services easy
#[derive(Clone)]
pub struct ServicesApiClient<C: Clone = HttpConnector> {
    client: Client<C>,
    base: Uri,
}

impl ServicesApiClient<HttpConnector> {
    fn new(base: Uri) -> Self {
        Self {
            client: Client::new(),
            base,
        }
    }
}

impl<C> ServicesApiClient<C>
where
    C: Connect + Clone + Send + Sync + 'static,
{
    fn builder(base: Uri, connector: C) -> Self {
        Self {
            client: Client::builder().build(connector),
            base,
        }
    }

    pub async fn request<B: Serialize, T: DeserializeOwned, H: Header>(
        &self,
        method: Method,
        path: &str,
        body: Option<B>,
        extra_header: Option<H>,
    ) -> Result<T, Error> {
        let bytes = self.request_raw(method, path, body, extra_header).await?;
        let json = serde_json::from_slice(&bytes)?;

        Ok(json)
    }

    pub async fn request_raw<B: Serialize, H: Header>(
        &self,
        method: Method,
        path: &str,
        body: Option<B>,
        extra_header: Option<H>,
    ) -> Result<Bytes, Error> {
        let uri = format!("{}{path}", self.base);
        trace!(uri, "calling inner service");

        let mut req = Request::builder().method(method).uri(uri);
        let headers = req
            .headers_mut()
            .expect("new request to have mutable headers");
        if let Some(extra_header) = extra_header {
            headers.typed_insert(extra_header);
        }
        if body.is_some() {
            headers.typed_insert(ContentType::json());
        }

        let cx = Span::current().context();
        global::get_text_map_propagator(|propagator| {
            propagator.inject_context(&cx, &mut HeaderInjector(req.headers_mut().unwrap()))
        });

        let req = if let Some(body) = body {
            req.body(Body::from(serde_json::to_vec(&body)?))
        } else {
            req.body(Body::empty())
        };

        let resp = self.client.request(req?).await?;
        trace!(response = ?resp, "Load response");

        if resp.status() != StatusCode::OK {
            return Err(Error::RequestError(resp.status()));
        }

        let bytes = body::to_bytes(resp.into_body()).await?;

        Ok(bytes)
    }
}

pub mod permit {
    use std::collections::HashMap;

    use headers::Authorization;
    use http::Method;
    use hyper::client::HttpConnector;
    use hyper_tls::HttpsConnector;
    use serde::{Serialize, Deserialize};
    use serde_json::{json, Value};
    use tracing::error;

    use super::ServicesApiClient;

    #[derive(Clone)]
    pub struct Client {
        client: ServicesApiClient<HttpsConnector<HttpConnector>>,
        client_pdp: ServicesApiClient<HttpsConnector<HttpConnector>>,
        api_key: String,
    }

    impl Client {
        pub fn new(api_key: &str) -> Self {
            Self {
                api_key: api_key.to_string(),
                client: ServicesApiClient::builder(
                    "https://api.eu-central-1.permit.io".parse().unwrap(),
                    HttpsConnector::new(),
                ),
                client_pdp: ServicesApiClient::builder(
                    "http://localhost:7000".parse().unwrap(),
                    HttpsConnector::new(),
                ),
            }
        }

        pub async fn create_project(&self, user_id: &str, project_id: &str) {
            // Need to create the resource first if the tenant is not given in the next call
            // let res: Result<Value, _> = self
            //     .client
            //     .request(
            //         Method::POST,
            //         "v2/facts/default/poc/resource_instances",
            //         Some(json!({
            //             "key": project_id,
            //             "tenant": "default",
            //             "resource": "Project",
            //         })),
            //         Some(Authorization::bearer(&self.api_key).unwrap()),
            //     )
            //     .await;

            // match res {
            //     Ok(r) => {
            //         dbg!(r);
            //     },
            //     Err(error) => {
            //         error!("failed to add project to permit: {error}");
            //     },
            // };

            let res: Result<Value, _> = self
                .client
                .request(
                    Method::POST,
                    "v2/facts/default/poc/role_assignments",
                    Some(json!({
                        "role": "admin",
                        "resource_instance": format!("Project:{project_id}"),
                        "tenant": "default",
                        "user": user_id,
                    })),
                    Some(Authorization::bearer(&self.api_key).unwrap()),
                )
                .await;

            match res {
                Ok(r) => {
                    // Success returns a 201 status code
                    dbg!(r);
                },
                Err(error) => {
                    error!("failed to assign user to project: {error}");
                },
            };
        }

        pub async fn delete_user_project(&self, user_id: &str, project_id: &str) {
            let res: Result<Value, _> = self
                .client
                .request(
                    Method::DELETE,
                    "v2/facts/default/poc/role_assignments",
                    Some(json!({
                        "role": "admin",
                        "resource_instance": format!("Project:{project_id}"),
                        "tenant": "default",
                        "user": user_id,
                    })),
                    Some(Authorization::bearer(&self.api_key).unwrap()),
                )
                .await;

            match res {
                Ok(r) => {
                    // Success returns a 201 status code
                    dbg!(r);
                },
                Err(error) => {
                    error!("failed to delete organization project in permit: {error}");
                },
            };
        }

        pub async fn create_organization(&self, user_id: &str, organization_name: &str) {
            // Assign the user to the org directly without creating the org first
            let res: Result<Value, _> = self
                .client
                .request(
                    Method::POST,
                    "v2/facts/default/poc/role_assignments",
                    Some(json!({
                        "role": "admin",
                        "resource_instance": format!("Organization:{organization_name}"),
                        "tenant": "default",
                        "user": user_id,
                    })),
                    Some(Authorization::bearer(&self.api_key).unwrap()),
                )
                .await;

            match res {
                Ok(r) => {
                    // Success returns a 201 status code
                    dbg!(r);
                },
                Err(error) => {
                    error!("failed to create organization in permit: {error}");
                },
            };
        }

        pub async fn delete_organization(&self, organization_id: &str) {
            let res: Result<Value, _> = self
                .client
                .request(
                    Method::DELETE,
                    &format!("v2/facts/default/poc/resource_instances/{organization_id}"),
                    None::<()>,
                    Some(Authorization::bearer(&self.api_key).unwrap()),
                )
                .await;

            match res {
                Ok(r) => {
                    // Success returns a 201 status code
                    dbg!(r);
                },
                Err(error) => {
                    error!("failed to delete organization in permit: {error}");
                },
            };
        }

        pub async fn get_organizations(&self, user_id: &str) -> Value {
             self
                .client
                .request(
                    Method::GET,
                    &format!("v2/facts/default/poc/role_assignments?user={user_id}&resource=Organization"),
                    None::<()>,
                    Some(Authorization::bearer(&self.api_key).unwrap()),
                )
                .await
                .unwrap()
        }

        pub async fn create_organization_project(&self, organization_name: &str, project_id: &str) {
            let res: Result<Value, _> = self
                .client
                .request(
                    Method::POST,
                    "v2/facts/default/poc/relationship_tuples",
                    Some(json!({
                        "subject": format!("Organization:{organization_name}"),
                        "tenant": "default",
                        "relation": "parent",
                        "object": format!("Project:{project_id}"),
                    })),
                    Some(Authorization::bearer(&self.api_key).unwrap()),
                )
                .await;

            match res {
                Ok(r) => {
                    // Success returns a 201 status code
                    dbg!(r);
                },
                Err(error) => {
                    error!("failed to create organization project in permit: {error}");
                },
            };
        }

        pub async fn delete_organization_project(&self, organization_name: &str, project_id: &str) {
            let res: Result<Value, _> = self
                .client
                .request(
                    Method::DELETE,
                    "v2/facts/default/poc/relationship_tuples",
                    Some(json!({
                        "subject": format!("Organization:{organization_name}"),
                        "relation": "parent",
                        "object": format!("Project:{project_id}"),
                    })),
                    Some(Authorization::bearer(&self.api_key).unwrap()),
                )
                .await;

            match res {
                Ok(r) => {
                    // Success returns a 201 status code
                    dbg!(r);
                },
                Err(error) => {
                    error!("failed to delete organization project in permit: {error}");
                },
            };
        }

        pub async fn get_organization_projects(&self, org_name: &str) -> Vec<OrganizationResource> {
             self
                .client
                .request(
                    Method::GET,
                    &format!("v2/facts/default/poc/relationship_tuples?subject=Organization:{org_name}&detailed=true"),
                    None::<()>,
                    Some(Authorization::bearer(&self.api_key).unwrap()),
                )
                .await
                .unwrap()
        }

        pub async fn get_organization_members(&self, org_name: &str) -> Vec<Value> {
             self
                .client
                .request(
                    Method::GET,
                    &format!("v2/facts/default/poc/role_assignments?resource_instance=Organization:{org_name}&role=member"),
                    None::<()>,
                    Some(Authorization::bearer(&self.api_key).unwrap()),
                )
                .await
                .unwrap()
        }

        pub async fn create_organization_member(&self, org_name: &str, user_id: &str) {
            let res: Result<Value, _> = self
                .client
                .request(
                    Method::POST,
                    "v2/facts/default/poc/role_assignments",
                    Some(json!({
                        "role": "member",
                        "resource_instance": format!("Organization:{org_name}"),
                        "tenant": "default",
                        "user": user_id,
                    })),
                    Some(Authorization::bearer(&self.api_key).unwrap()),
                )
                .await;

            match res {
                Ok(r) => {
                    // Success returns a 201 status code
                    dbg!(r);
                },
                Err(error) => {
                    error!("failed to delete organization member in permit: {error}");
                },
            };
        }

        pub async fn delete_organization_member(&self, org_name: &str, user_id: &str) {
            let res: Result<Value, _> = self
                .client
                .request(
                    Method::DELETE,
                    "v2/facts/default/poc/role_assignments",
                    Some(json!({
                        "role": "member",
                        "resource_instance": format!("Organization:{org_name}"),
                        "tenant": "default",
                        "user": user_id,
                    })),
                    Some(Authorization::bearer(&self.api_key).unwrap()),
                )
                .await;

            match res {
                Ok(r) => {
                    // Success returns a 201 status code
                    dbg!(r);
                },
                Err(error) => {
                    error!("failed to delete organization member in permit: {error}");
                },
            };
        }

        pub async fn get_user_projects(&self, user_id: &str) -> Vec<ProjectPermissions> {
             let perms: HashMap<String, ProjectPermissions> = self.client_pdp
                .request(
                    Method::POST,
                    &format!("user-permissions"),
                    Some(json!({
                        "user": {"key": user_id},
                        "resource_types": ["Project"],
                    })),
                    Some(Authorization::bearer(&self.api_key).unwrap()),
                )
                .await
                .unwrap();

            perms.into_values().collect()
        }

        pub async fn allowed(&self, user_id: &str, project_id: &str, action: &str) -> bool {
            let res: Result<Value, _> = self
                .client_pdp
                .request(
                    Method::POST,
                    &format!("allowed"),
                    Some(json!({
                        "user": {"key": user_id},
                        "action": action,
                        "resource": {"type": "Project", "key": project_id, "tenant": "default"},
                    })),
                    Some(Authorization::bearer(&self.api_key).unwrap()),
                )
                .await;

            match res {
                Ok(r) => {
                    // Success returns a 201 status code
                    dbg!(&r);
                    r["allow"].as_bool().unwrap()
                },
                Err(error) => {
                    error!("failed to get user permissions in permit: {error}");
                    false
                },
            }
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
}

#[cfg(test)]
mod tests {
    use headers::{authorization::Bearer, Authorization};
    use http::{Method, StatusCode};

    use crate::models;
    use crate::test_utils::get_mocked_gateway_server;

    use super::{Error, ServicesApiClient};

    // Make sure we handle any unexpected responses correctly
    #[tokio::test]
    async fn api_errors() {
        let server = get_mocked_gateway_server().await;

        let client = ServicesApiClient::new(server.uri().parse().unwrap());

        let err = client
            .request::<_, Vec<models::project::Response>, _>(
                Method::GET,
                "projects",
                None::<()>,
                None::<Authorization<Bearer>>,
            )
            .await
            .unwrap_err();

        assert!(matches!(err, Error::RequestError(StatusCode::UNAUTHORIZED)));
    }
}
