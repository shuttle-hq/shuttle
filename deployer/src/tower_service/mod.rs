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

    fn get<B>(&self, _path: &str) -> ResponseFuture<http::Response<B>, anyhow::Error> {
        Box::pin(async { Err(anyhow!("TODO")) })
    }

    fn post<B>(&self, _path: &str) -> ResponseFuture<http::Response<B>, anyhow::Error> {
        Box::pin(async { Err(anyhow!("TODO")) })
    }
}

impl<Body> tower::Service<http::Request<Body>> for Deployer {
    type Response = http::Response<Body>;
    type Error = anyhow::Error;
    type Future = ResponseFuture<Self::Response, Self::Error>;

    fn poll_ready(&mut self, _cx: &mut Context) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: http::Request<Body>) -> Self::Future {
        let path = req.uri().path();

        match req.method() {
            &http::Method::GET => self.get(path),
            &http::Method::POST => self.post(path),
            unexpected => {
                let method_string = unexpected.to_string();
                Box::pin(async move {
                    Err(anyhow!("Received request with unexpected HTTP method: {}", method_string))
                })
            }
        }
    }
}

type ResponseFuture<Resp, Error> = Pin<Box<dyn Future<Output = Result<Resp, Error>> + Send + Sync>>;
