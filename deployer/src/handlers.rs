use axum::body::Body;
use axum::extract::{Extension, Path, Query};
use axum::routing::{get, Router};
use axum::{extract::BodyStream, Json};
use futures::TryStreamExt;

use crate::deployment::{DeploymentInfo, DeploymentManager, Queued};
use crate::error::{Error, Result};
use crate::persistence::Persistence;

use std::collections::HashMap;

pub fn make_router(
    persistence: Persistence,
    deployment_manager: DeploymentManager,
) -> Router<Body> {
    Router::new()
        .route("/services", get(list_services))
        .route(
            "/services/:name",
            get(get_service).post(post_service).delete(delete_service),
        )
        .layer(Extension(persistence))
        .layer(Extension(deployment_manager))
}

async fn list_services(
    Extension(persistence): Extension<Persistence>,
) -> Result<Json<Vec<DeploymentInfo>>> {
    persistence.get_all_deployments().await.map(Json)
}

async fn get_service(
    Extension(persistence): Extension<Persistence>,
    Path(name): Path<String>,
) -> Result<Json<Option<DeploymentInfo>>> {
    persistence.get_deployment(&name).await.map(Json)
}

async fn post_service(
    Extension(deployment_manager): Extension<DeploymentManager>,
    Path(name): Path<String>,
    Query(params): Query<HashMap<String, String>>,
    stream: BodyStream,
) -> Result<Json<DeploymentInfo>> {
    let queued = Queued {
        name,
        data_stream: Box::pin(stream.map_err(Error::Streaming)),
        will_run_tests: !params.contains_key("no-testing"),
    };
    let info = DeploymentInfo::from(&queued);

    deployment_manager.queue_push(queued).await;

    Ok(Json(info))
}

async fn delete_service(
    Extension(persistence): Extension<Persistence>,
    Extension(deployment_manager): Extension<DeploymentManager>,
    Path(name): Path<String>,
) -> Result<Json<Option<DeploymentInfo>>> {
    let old_info = persistence.delete_deployment(&name).await?;
    deployment_manager.kill(name).await;

    Ok(Json(old_info))
}
