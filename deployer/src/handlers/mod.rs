use axum::{handler::Handler, headers::HeaderMapExt, routing::post, Router};
use http::Uri;
use shuttle_common::{
    backends::{
        auth::{AuthPublicKey, JwtAuthenticationLayer, ScopedLayer},
        headers::XShuttleAccountName,
        metrics::TraceLayer,
    },
    claims::Scope,
    request_span,
};

use tracing::field;

mod deployment;
mod error;

pub async fn make_router(auth_uri: Uri) -> Router {
    Router::new()
        .route(
            "/deploy/:project_name",
            post(deployment::deploy_project.layer(ScopedLayer::new(vec![Scope::DeploymentPush]))),
        )
        .layer(JwtAuthenticationLayer::new(AuthPublicKey::new(auth_uri)))
        // This route should be below the auth bearer since it does not need authentication
        .layer(
            TraceLayer::new(|request| {
                let account_name = request
                    .headers()
                    .typed_get::<XShuttleAccountName>()
                    .unwrap_or_default();

                request_span!(
                    request,
                    account.name = account_name.0,
                    request.params.project_name = field::Empty,
                )
            })
            .with_propagation()
            .build(),
        )
}
