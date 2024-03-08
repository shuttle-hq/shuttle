use std::sync::Arc;

use axum::response::Response;
use http::{HeaderMap, Method, Request, StatusCode, Uri};
use hyper::Body;
use serde::de::DeserializeOwned;
use shuttle_common::{
    models::{deployment, error::ErrorKind, project::ProjectName, user::UserId},
    resource,
};
use uuid::Uuid;

use crate::{auth::ScopedUser, project::Project, service::GatewayService, Error};

use super::latest::RouterState;

/// Helper to easily make requests to a project
pub(crate) struct ProjectCaller {
    project: Project,
    project_name: ProjectName,
    user_id: UserId,
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
        let project = service
            .find_or_start_project(&project_name, sender)
            .await?
            .0;

        Ok(Self {
            project: project.state,
            project_name,
            user_id: scoped_user.user.id,
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
            .route(&self.project, &self.project_name, &self.user_id, req)
            .await
    }

    /// Make a request call and deserialize the body to the generic type
    /// Returns `None` when the request was successful but found nothing
    async fn call_deserialize<T: DeserializeOwned>(
        &self,
        uri: &str,
        method: Method,
    ) -> Result<Option<T>, Error> {
        let res = self.call(uri, method).await?;

        match res.status() {
            StatusCode::NOT_FOUND => Ok(None),
            StatusCode::OK => {
                let body_bytes = hyper::body::to_bytes(res.into_body())
                    .await
                    .map_err(|e| Error::source(ErrorKind::Internal, e))?;
                let body = serde_json::from_slice(&body_bytes)
                    .map_err(|e| Error::source(ErrorKind::Internal, e))?;

                Ok(Some(body))
            }
            _ => Err(Error::from_kind(ErrorKind::Internal)),
        }
    }

    /// Get the deployments for the project
    pub async fn get_deployment_list(&self) -> Result<Vec<deployment::Response>, Error> {
        let project_name = &self.project_name;

        let deployments = self
            .call_deserialize(
                &format!("/projects/{project_name}/deployments"),
                Method::GET,
            )
            .await?;

        Ok(deployments.unwrap_or_default())
    }

    /// Stop a deployment of the project
    pub async fn stop_deployment(&self, deployment_id: &Uuid) -> Result<Response<Body>, Error> {
        let project_name = &self.project_name;

        self.call(
            &format!("/projects/{project_name}/deployments/{deployment_id}"),
            Method::DELETE,
        )
        .await
    }

    /// Get all the resources the project is using
    pub async fn get_resources(&self) -> Result<Vec<resource::Response>, Error> {
        let project_name = &self.project_name;

        let resources = self
            .call_deserialize(
                &format!("/projects/{project_name}/services/{project_name}/resources"),
                Method::GET,
            )
            .await?;

        Ok(resources.unwrap_or_default())
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
