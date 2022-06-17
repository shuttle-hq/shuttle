use axum::body::Body;
use axum::extract::{Extension, Path};
use axum::routing::{get, Router};
use axum::{extract::BodyStream, Json};

use crate::deployment::{DeploymentInfo, DeploymentManager, DeploymentState, Queued};
use crate::persistence::Persistence;

type Error = String;

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
) -> Result<Json<Vec<DeploymentInfo>>, Error> {
    persistence
        .get_all_deployments()
        .await
        .map(Json)
        .map_err(|e| e.to_string())
}

async fn get_service(
    Extension(persistence): Extension<Persistence>,
    Path(name): Path<String>,
) -> Result<Json<DeploymentInfo>, Error> {
    persistence
        .get_deployment(&name)
        .await
        .map(Json)
        .map_err(|e| e.to_string())
}

async fn post_service(
    Extension(persistence): Extension<Persistence>,
    Extension(deployment_manager): Extension<DeploymentManager>,
    Path(name): Path<String>,
    stream: BodyStream,
) -> Result<Json<DeploymentInfo>, Error> {
    let queued = Queued {
        name,
        state: DeploymentState::Queued,
        data_stream: Box::pin(stream), // TODO: Map error type
    };
    let info = DeploymentInfo::from(&queued);

    persistence
        .update_deployment(&queued)
        .await
        .map_err(|e| e.to_string())?;
    deployment_manager.queue_push(queued).await;

    Ok(Json(info))
}

async fn delete_service(
    Extension(persistence): Extension<Persistence>,
    Extension(deployment_manager): Extension<DeploymentManager>,
    Path(name): Path<String>,
) -> Result<Json<DeploymentInfo>, Error> {
    let old_info = persistence
        .delete_deployment(&name)
        .await
        .map_err(|e| e.to_string())?;
    deployment_manager.kill(name).await;

    Ok(Json(old_info))
}
