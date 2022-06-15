pub mod middleware;

use crate::deployment::{DeploymentManager, DeploymentState, Queued};
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
    ) -> Result<http::Response<Body>, anyhow::Error>
    where
        Body: HttpBody + Send + Sync + 'static,
        <Body as HttpBody>::Data: Send + Sync,
        <Body as HttpBody>::Error: Display,
    {
        match method {
            http::Method::GET => todo!(),

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

                // Store deployment state:
                self.persistence.deployment(&queued).await?;

                // Add to build queue:
                self.deployment_manager.queue_push(queued).await;

                // Produce response:
                Err(anyhow!("TODO"))

                /*Ok(http::Response::builder()
                .status(http::StatusCode::OK)
                .body(body)
                .unwrap())*/
            }

            http::Method::DELETE => todo!(),

            unexpected => {
                let method_string = unexpected.to_string();
                Err(anyhow!(
                    "Unexpected HTTP method for service access: {}",
                    method_string
                ))
            }
        }
    }
}

impl<Body> tower::Service<http::Request<Body>> for Deployer
where
    Body: HttpBody + Sync + Send + 'static,
    <Body as HttpBody>::Data: Send + Sync,
    <Body as HttpBody>::Error: Display,
{
    type Response = http::Response<Body>;
    type Error = anyhow::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, _cx: &mut Context) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: http::Request<Body>) -> Self::Future {
        let path = req.uri().path().to_string();
        let method = req.method().clone();
        let body = req.into_body();

        if let Some(groups) = SERVICES_SLASH_NAME_RE.captures(&path) {
            let service_name = groups.get(1).unwrap().as_str().to_string();
            let cloned = self.clone(); // TODO: Work about appropriate lifetimes to avoid cloning

            return Box::pin(async move {
                cloned
                    .access_service::<Body>(service_name, method, body)
                    .await
            });
        }

        Box::pin(async move { Err(anyhow!("Unexpected HTTP request path: {}", path)) })
    }
}
