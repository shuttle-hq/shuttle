use std::{
    fs,
    net::{Ipv4Addr, SocketAddr},
    path::Path,
};

use portpicker::pick_unused_port;
use pretty_assertions::assert_eq;
use shuttle_builder::Service;
use shuttle_common::claims::{Claim, Scope};
use shuttle_proto::builder::{
    build_response::Secret, builder_client::BuilderClient, builder_server::BuilderServer,
    BuildRequest,
};
use tokio::select;
use tonic::{transport::Server, Request};
use ulid::Ulid;

#[tokio::test]
async fn build_crate() {
    let port = pick_unused_port().unwrap();
    let addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), port);

    let server_future = async {
        Server::builder()
            .layer(JwtScopesLayer::new(vec![Scope::DeploymentPush]))
            .add_service(BuilderServer::new(Service::new()))
            .serve(addr)
            .await
            .unwrap()
    };

    let test_future = async {
        let mut client = BuilderClient::connect(format!("http://localhost:{port}"))
            .await
            .unwrap();
        let resources = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("resources");

        // Build a normal hello world archive
        let deployment_id = Ulid::new().to_string();
        let archive = fs::read(resources.join("hello-world-0.1.0.crate")).unwrap();

        let response = client
            .build(Request::new(BuildRequest {
                deployment_id: deployment_id.clone(),
                archive,
            }))
            .await
            .unwrap()
            .into_inner();

        assert_eq!(response.is_wasm, false);
        assert_eq!(response.secrets, Vec::new());

        // Build an archive with secrets
        let deployment_id = Ulid::new().to_string();
        let archive = fs::read(resources.join("secrets-0.1.0.crate")).unwrap();

        let response = client
            .build(Request::new(BuildRequest {
                deployment_id: deployment_id.clone(),
                archive,
            }))
            .await
            .unwrap()
            .into_inner();

        assert_eq!(response.is_wasm, false);
        assert_eq!(
            response.secrets,
            vec![Secret {
                key: "MY_API_KEY".to_string(),
                value: "the contents of my API key".to_string()
            }]
        );

        // Build a workspace archive
        // TODO: add workspace support to nbuild
        // let deployment_id = Ulid::new().to_string();
        // let archive = fs::read(resources.join("workspace-0.1.0.crate")).unwrap();

        // let response = client
        //     .build(Request::new(BuildRequest {
        //         deployment_id: deployment_id.clone(),
        //         archive,
        //     }))
        //     .await
        //     .unwrap()
        //     .into_inner();

        // assert_eq!(response.is_wasm, false);
        // assert_eq!(response.secrets, Vec::new(),);

        // Build a wasm archive
        // TODO: add target support to nbuild
        // let deployment_id = Ulid::new().to_string();
        // let archive = fs::read(resources.join("wasm-0.1.0.crate")).unwrap();

        // let response = client
        //     .build(Request::new(BuildRequest {
        //         deployment_id: deployment_id.clone(),
        //         archive,
        //     }))
        //     .await
        //     .unwrap()
        //     .into_inner();

        // assert_eq!(response.is_wasm, true);
        // assert_eq!(response.secrets, Vec::new(),);
    };

    select! {
        _ = server_future => panic!("server finished first"),
        _ = test_future => {},
    }
}

/// Layer to set JwtScopes on a request
#[derive(Clone)]
pub struct JwtScopesLayer {
    /// Thes scopes to set
    scopes: Vec<Scope>,
}

impl JwtScopesLayer {
    /// Create a new layer to set scopes on requests
    pub fn new(scopes: Vec<Scope>) -> Self {
        Self { scopes }
    }
}

impl<S> tower::Layer<S> for JwtScopesLayer {
    type Service = JwtScopes<S>;

    fn layer(&self, inner: S) -> Self::Service {
        JwtScopes {
            inner,
            scopes: self.scopes.clone(),
        }
    }
}

/// Middleware to set scopes on a request
#[derive(Clone)]
pub struct JwtScopes<S> {
    inner: S,
    scopes: Vec<Scope>,
}

impl<S> tower::Service<hyper::Request<hyper::Body>> for JwtScopes<S>
where
    S: tower::Service<hyper::Request<hyper::Body>> + Send + Clone + 'static,
    S::Future: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = S::Future;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut req: hyper::Request<hyper::Body>) -> Self::Future {
        req.extensions_mut()
            .insert(Claim::new("test".to_string(), self.scopes.clone()));
        self.inner.call(req)
    }
}
