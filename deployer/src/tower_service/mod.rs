pub mod middleware;

use crate::deployment::DeploymentManager;
use crate::persistence::Persistence;

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use anyhow::anyhow;

#[derive(Clone)]
pub struct Deployer {
    deployment_manager: DeploymentManager,
    persistence: Persistence,
}

impl Deployer {
    pub async fn new() -> Self {
        Deployer { deployment_manager: DeploymentManager::new(), persistence: Persistence::new().await }
    }
}

impl<Body> tower::Service<http::Request<Body>> for Deployer {
    type Response = http::Response<Body>;
    type Error = anyhow::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send + Sync>>;

    fn poll_ready(&mut self, _cx: &mut Context) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, _req: http::Request<Body>) -> Self::Future {
        Box::pin(async { Err(anyhow!("TODO")) })
    }
}
