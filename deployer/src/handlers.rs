use axum::body::Body;
use axum::extract::{Extension, Path, Query};
use axum::routing::{get, Router};
use axum::{extract::BodyStream, Json};
use chrono::Utc;
use futures::TryStreamExt;
use uuid::Uuid;

use crate::deployment::{DeploymentManager, Queued, State};
use crate::error::{Error, Result};
use crate::persistence::{Deployment, Persistence};

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
        .route(
            "/deployments/:id",
            get(get_deployment).delete(delete_deployment),
        )
        .layer(Extension(persistence))
        .layer(Extension(deployment_manager))
}

async fn list_services(
    Extension(persistence): Extension<Persistence>,
) -> Result<Json<Vec<String>>> {
    persistence.get_all_services().await.map(Json)
}

async fn get_service(
    Extension(persistence): Extension<Persistence>,
    Path(name): Path<String>,
) -> Result<Json<Vec<Deployment>>> {
    persistence.get_deployments(&name).await.map(Json)
}

async fn post_service(
    Extension(persistence): Extension<Persistence>,
    Extension(deployment_manager): Extension<DeploymentManager>,
    Path(name): Path<String>,
    Query(params): Query<HashMap<String, String>>,
    stream: BodyStream,
) -> Result<Json<Deployment>> {
    let id = Uuid::new_v4();

    let deployment = Deployment {
        id: id.clone(),
        name: name.clone(),
        state: State::Queued,
        last_update: Utc::now(),
    };

    persistence.insert_deployment(deployment.clone()).await?;

    let queued = Queued {
        id,
        name,
        data_stream: Box::pin(stream.map_err(Error::Streaming)),
        will_run_tests: !params.contains_key("no-testing"),
    };

    deployment_manager.queue_push(queued).await;

    Ok(Json(deployment))
}

async fn delete_service(
    Extension(persistence): Extension<Persistence>,
    Extension(deployment_manager): Extension<DeploymentManager>,
    Path(name): Path<String>,
) -> Result<Json<Vec<Deployment>>> {
    let old_deployments = persistence.delete_service(&name).await?;

    for deployment in old_deployments.iter() {
        deployment_manager.kill(deployment.id).await;
    }

    Ok(Json(old_deployments))
}

async fn get_deployment(
    Extension(persistence): Extension<Persistence>,
    Path(id): Path<Uuid>,
) -> Result<Json<Option<Deployment>>> {
    persistence.get_deployment(&id).await.map(Json)
}

async fn delete_deployment(
    Extension(persistence): Extension<Persistence>,
    Extension(deployment_manager): Extension<DeploymentManager>,
    Path(id): Path<Uuid>,
) -> Result<Json<Option<Deployment>>> {
    deployment_manager.kill(id).await;

    persistence.get_deployment(&id).await.map(Json)
}
