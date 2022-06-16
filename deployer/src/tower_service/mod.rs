pub mod middleware;

use crate::deployment::{Built, DeploymentInfo, DeploymentManager, DeploymentState, Queued};
use crate::persistence::Persistence;

use std::fmt::Display;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use anyhow::anyhow;
use futures::FutureExt;
use hyper::body::HttpBody;
use lazy_static::lazy_static;
use regex::Regex;

lazy_static! {
    static ref SERVICES_SLASH_NAME_RE: Regex = Regex::new("^/services/([a-zA-|0-9_]+)$").unwrap();
}

#[derive(Clone)]
pub struct Deployer {
    deployment_manager: DeploymentManager,
    persistence: Persistence,
}

impl Deployer {
    pub async fn new() -> Self {
        let persistence = Persistence::new().await;

        let cpus = num_cpus::get();
        let pipeline_count = (cpus + 2) / 3; // TODO: How many is suitable?
        let deployment_manager = DeploymentManager::new(persistence.clone(), pipeline_count);

        // Any deployments already built? Start 'em up!

        let runnable_deployments = persistence.get_all_runnable_deployments().await.unwrap();

        for deployment in runnable_deployments {
            let built = Built {
                name: deployment.name,
                state: DeploymentState::Built,
            };
            deployment_manager.run_push(built).await;
        }

        Deployer {
            deployment_manager,
            persistence,
        }
    }

    async fn access_service<Body>(
        &self,
        name: String,
        method: http::Method,
        body: Body,
    ) -> Result<serde_json::Value, anyhow::Error>
    where
        Body: HttpBody + Send + Sync + 'static,
        <Body as HttpBody>::Data: Send + Sync,
        <Body as HttpBody>::Error: Display,
    {
        match method {
            http::Method::GET => {
                let deployment = self.persistence.get_deployment(&name).await?;
                Ok(serde_json::to_value(deployment).unwrap())
            }

            http::Method::POST => {
                let data_future = Box::pin(hyper::body::to_bytes(body).map(|res| {
                    res.map(|data| data.to_vec())
                        .map_err(|e| anyhow!("Failed to read service POST request body: {}", e))
                }));

                let queued = Queued {
                    name,
                    data_future,
                    state: DeploymentState::Queued,
                };
                let info = DeploymentInfo::from(&queued);

                // Store deployment state:
                self.persistence.update_deployment(&queued).await?;

                // Add to build queue:
                self.deployment_manager.queue_push(queued).await;

                // Produce response:
                Ok(serde_json::to_value(info).unwrap())
            }

            http::Method::DELETE => {
                let deleted = self.persistence.delete_deployment(&name).await?;

                // Stop task in which the service is executing:
                // TODO

                Ok(serde_json::to_value(deleted).unwrap())
            }

            unexpected => {
                let method_string = unexpected.to_string();
                Err(anyhow!(
                    "Unexpected HTTP method for service access: {}",
                    method_string
                ))
            }
        }
    }

    async fn list_services<Body>(&self) -> anyhow::Result<serde_json::Value> {
        let deployments = self.persistence.get_all_deployments().await?;
        Ok(serde_json::to_value(deployments).unwrap())
    }
}

impl<Body> tower::Service<http::Request<Body>> for Deployer
where
    Body: HttpBody + Sync + Send + 'static,
    <Body as HttpBody>::Data: Send + Sync,
    <Body as HttpBody>::Error: Display,
{
    type Response = http::Response<String>;
    type Error = anyhow::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, _cx: &mut Context) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: http::Request<Body>) -> Self::Future {
        let path = req.uri().path().to_string();
        let method = req.method().clone();
        let body = req.into_body();

        let cloned = self.clone(); // TODO: Work about appropriate lifetimes to avoid cloning

        let resp = http::Response::builder().header("Content-Type", "application/json");

        if let Some(groups) = SERVICES_SLASH_NAME_RE.captures(&path) {
            let service_name = groups.get(1).unwrap().as_str().to_string();

            return Box::pin(async move {
                cloned
                    .access_service(service_name, method, body)
                    .await
                    .map(|json| resp.body(json.to_string()).unwrap())
            });
        }

        if path == "/services" {
            return Box::pin(async move {
                cloned
                    .list_services::<Body>()
                    .await
                    .map(|json| resp.body(json.to_string()).unwrap())
            });
        }

        Box::pin(async move { Err(anyhow!("Unexpected HTTP request path: {}", path)) })
    }
}
