use crate::deployment::{DeploymentInfo, DeploymentManager, DeploymentState, Queued, BuildLogReceiver};
use crate::error::{Error, Result};
use crate::persistence::Persistence;

use axum::extract::{Extension, Path, BodyStream};
use axum::extract::ws;
use axum::routing::{get, Router};
use axum::body::Body;
use axum::Json;

use futures::TryStreamExt;
use tokio::sync::broadcast;

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
        .route("/services/:name/build-logs", get(subscribe_build_logs))
        .layer(Extension(persistence))
        .layer(Extension(deployment_manager))
        .layer(Extension(BuildLogReceivers::new()))
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
    Extension(persistence): Extension<Persistence>,
    Extension(deployment_manager): Extension<DeploymentManager>,
    Extension(mut build_log_receivers): Extension<BuildLogReceivers>,
    Path(name): Path<String>,
    stream: BodyStream,
) -> Result<Json<DeploymentInfo>> {
    let (build_log_sender, build_log_recv) = broadcast::channel(10);

    build_log_receivers.insert(name.clone(), build_log_recv);

    let queued = Queued {
        name,
        state: DeploymentState::Queued,
        data_stream: Box::pin(stream.map_err(Error::Streaming)),
        build_log_sender,
    };
    let info = DeploymentInfo::from(&queued);

    persistence.update_deployment(&queued).await?;
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

async fn subscribe_build_logs(Extension(mut build_log_receivers): Extension<BuildLogReceivers>, Path(name): Path<String>, ws_upgrade: ws::WebSocketUpgrade) -> axum::response::Response {
    let mut log_recv = build_log_receivers.get(&name).unwrap().clone(); // TODO: Error handling.

    ws_upgrade.on_upgrade(move |mut s| {
        async move {
            while let Ok(msg) = log_recv.recv().await {
                if s.send(ws::Message::Text(msg)).await.is_err() {
                    break; // client disconnected
                }
            }

            build_log_receivers.remove(&name);
        }
    })
}

type BuildLogReceivers = HashMap<String, BuildLogReceiver>;
