use std::sync::Arc;

use axum::response::Response;
use http::{HeaderMap, Method, Request, StatusCode, Uri};
use hyper::Body;
use serde::de::DeserializeOwned;
use shuttle_common::{
    models::{error::ErrorKind, project::ProjectName, service},
    resource,
};

use crate::{auth::ScopedUser, project::Project, service::GatewayService, AccountName, Error};

use super::latest::RouterState;

/// Helper to easily make requests to a project
pub struct ProjectCaller {
    project: Project,
    project_name: ProjectName,
    account_name: AccountName,
    service: Arc<GatewayService>,
    headers: HeaderMap,
}

impl ProjectCaller {
    /// Make a new project caller to easily make requests to this project
    pub async fn new(
        state: RouterState,
        scoped_user: ScopedUser,
        headers: &HeaderMap,
    ) -> Result<Self, Error> {
        let RouterState {
            service, sender, ..
        } = state;
        let project_name = scoped_user.scope;
        let project = service.find_or_start_project(&project_name, sender).await?;

        Ok(Self {
            project: project.state,
            project_name,
            account_name: scoped_user.user.name,
            service,
            headers: headers.clone(),
        })
    }

    /// Make a simple request call to get the response
    pub async fn call(&self, uri: &str, method: Method) -> Result<Response<Body>, Error> {
        let mut rb = Request::builder();
        rb.headers_mut().unwrap().clone_from(&self.headers);
        let req = rb
            .uri(uri.parse::<Uri>().unwrap())
            .method(method)
            .body(hyper::Body::empty())
            .unwrap();

        self.service
            .route(&self.project, &self.project_name, &self.account_name, req)
            .await
    }

    /// Make a request call and deserialize the body to the generic type
    async fn call_deserialize<T: DeserializeOwned + Default>(
        &self,
        uri: &str,
        method: Method,
    ) -> Result<T, Error> {
        let res = self.call(uri, method).await?;

        if res.status() != StatusCode::NOT_FOUND {
            if res.status() != StatusCode::OK {
                return Err(Error::from_kind(ErrorKind::Internal));
            }
            let body_bytes = hyper::body::to_bytes(res.into_body())
                .await
                .map_err(|e| Error::source(ErrorKind::Internal, e))?;
            let body = serde_json::from_slice(&body_bytes)
                .map_err(|e| Error::source(ErrorKind::Internal, e))?;

            Ok(body)
        } else {
            Ok(Default::default())
        }
    }

    /// Get the service summary for the project
    pub async fn get_service_summary(&self) -> Result<service::Summary, Error> {
        let project_name = &self.project_name;

        self.call_deserialize(
            &format!("/projects/{project_name}/services/{project_name}"),
            Method::GET,
        )
        .await
    }

    /// Stop the active deployment of the project
    pub async fn stop_active_deployment(&self) -> Result<Response<Body>, Error> {
        let project_name = &self.project_name;

        self.call(
            &format!("/projects/{project_name}/services/{project_name}"),
            Method::DELETE,
        )
        .await
    }

    /// Get all the resources the project is using
    pub async fn get_resources(&self) -> Result<Vec<resource::Response>, Error> {
        let project_name = &self.project_name;

        self.call_deserialize(
            &format!("/projects/{project_name}/services/{project_name}/resources"),
            Method::GET,
        )
        .await
    }

    /// Delete a resource used by the project
    pub async fn delete_resource(&self, r#type: &str) -> Result<Response<Body>, Error> {
        let project_name = &self.project_name;

        self.call(
            &format!("/projects/{project_name}/services/{project_name}/resources/{type}"),
            Method::DELETE,
        )
        .await
    }
}
