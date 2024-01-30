use std::{net::SocketAddr, sync::Arc};

use axum::{middleware::from_extractor, routing::get, Extension, Router};
use futures::Future;
use http::Uri;
use shuttle_common::{
    backends::{
        auth::{AuthPublicKey, JwtAuthenticationLayer},
        metrics::{Metrics, TraceLayer},
    },
    request_span,
};
use tracing::field;

use crate::service::GatewayService;

use self::handlers::get_service;

pub mod authz;
pub mod handlers;

#[derive(Clone)]
pub struct DeployerApiState {
    pub service: Arc<GatewayService>,
}

pub struct Builder {
    router: Router<DeployerApiState>,
    service: Option<Arc<GatewayService>>,
    bind: Option<SocketAddr>,
}

impl Builder {
    pub fn with_binding(mut self, bind: SocketAddr) -> Self {
        self.bind = Some(bind);
        self
    }

    pub fn with_service(mut self, service: Arc<GatewayService>) -> Self {
        self.service = Some(service);
        self
    }

    pub fn with_jwt_guarded_routes(mut self, auth_uri: Uri) -> Self {
        let auth_public_key = AuthPublicKey::new(auth_uri.clone());
        self.router = self
            .router
            // TODO add more routes
            .layer(JwtAuthenticationLayer::new(auth_public_key));
        self
    }

    pub fn with_admin_secret_guarded_routes(mut self) -> Self {
        self.router = self.router.route(
            "/projects/:project_name/services/:service_name",
            get(get_service),
        );

        self
    }

    pub fn with_default_traces(mut self) -> Self {
        self.router = self.router.route_layer(from_extractor::<Metrics>()).layer(
            TraceLayer::new(|request| {
                request_span!(
                    request,
                    account.name = field::Empty,
                    request.params.project_name = field::Empty,
                    request.params.account_name = field::Empty
                )
            })
            .with_propagation()
            .build(),
        );
        self
    }

    pub fn into_router(self) -> Router {
        let service = self.service.expect("a service to be set");
        self.router
            .with_state(DeployerApiState {
                service: service.clone(),
            })
            .layer(Extension(service.db.clone()))
    }

    pub fn serve(self) -> impl Future<Output = Result<(), hyper::Error>> {
        let bind = self.bind.expect("a socket address to bind to is required");
        let router = self.into_router();
        axum::Server::bind(&bind).serve(router.into_make_service())
    }
}

impl Default for Builder {
    fn default() -> Self {
        Builder {
            router: Router::new(),
            service: None,
            bind: None,
        }
    }
}
