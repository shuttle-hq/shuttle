use std::path::PathBuf;

use axum::extract::Extension;
use axum::handler::Handler;
use axum::middleware;
use axum::routing::{post, Router};
use http::Uri;
use shuttle_common::{
    backends::auth::{AuthPublicKey, JwtAuthenticationLayer, ScopedLayer},
    claims::Scope,
};
use tracing::warn;

use crate::builder::MockedBuilder;

pub mod error;
mod local;

#[derive(Clone)]
pub struct RouterBuilder {
    router: Router,
    auth_uri: Uri,
}

impl RouterBuilder {
    pub fn new(auth_uri: &Uri) -> Self {
        let router = Router::new()
            .route(
                "/project/:project_name",
                post(
                    super::api::deploy_project.layer(ScopedLayer::new(vec![Scope::DeploymentPush])),
                ),
            )
            .layer(JwtAuthenticationLayer::new(AuthPublicKey::new(
                auth_uri.clone(),
            )));

        Self {
            router,
            auth_uri: auth_uri.clone(),
        }
    }

    pub fn with_mocked_builder_image_archive_path(mut self, path: PathBuf) -> Self {
        self.router = self.router.layer(Extension(MockedBuilder::new(path)));

        self
    }

    /// Sets an admin JWT bearer token on every request for use when running deployer locally.
    pub fn with_local_admin_layer(mut self) -> Self {
        warn!("Building deployer router with auth bypassed, this should only be used for local development.");
        self.router = self
            .router
            .layer(middleware::from_fn(local::set_jwt_bearer))
            .layer(Extension(self.auth_uri.clone()));

        self
    }

    pub fn into_router(self) -> Router {
        self.router
    }
}
