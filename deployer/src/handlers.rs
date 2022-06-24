use crate::deployment::{
    BuildLogReceiver, DeploymentInfo, DeploymentManager, DeploymentState, Queued,
};
use crate::error::{Error, Result};
use crate::persistence::Persistence;

use axum::body::Body;
use axum::extract::ws;
use axum::extract::{ws::WebSocket, BodyStream, Extension, Path};
use axum::routing::{get, Router};
use axum::Json;

use futures::TryStreamExt;
use tokio::sync::{mpsc, Mutex};

use std::collections::HashMap;
use std::sync::Arc;

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
        .layer(Extension(Arc::new(Mutex::new(BuildLogReceivers::new()))))
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
    Extension(build_log_receivers): Extension<Arc<Mutex<BuildLogReceivers>>>,
    Path(name): Path<String>,
    stream: BodyStream,
) -> Result<Json<DeploymentInfo>> {
    let (build_log_sender, build_log_receiver) = mpsc::channel(10);

    build_log_receivers
        .lock()
        .await
        .insert(name.clone(), build_log_receiver);

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

async fn subscribe_build_logs(
    Extension(build_log_receivers): Extension<Arc<Mutex<BuildLogReceivers>>>,
    Path(name): Path<String>,
    ws_upgrade: ws::WebSocketUpgrade,
) -> axum::response::Response {
    // TODO: Error handling.
    let log_recv = build_log_receivers.lock().await.remove(&name).unwrap();

    ws_upgrade.on_upgrade(move |s| websocket_handler(s, log_recv))
}

async fn websocket_handler(mut s: WebSocket, mut log_recv: BuildLogReceiver) {
    while let Some(msg) = log_recv.recv().await {
        if s.send(ws::Message::Text(msg)).await.is_err() {
            return; // client disconnected
        }
    }
    let _ = s.close().await;
}

type BuildLogReceivers = HashMap<String, BuildLogReceiver>;
