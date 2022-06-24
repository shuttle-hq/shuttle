use crate::deployment::{
    BuildLogReceiver, BuildLogsManager, DeploymentInfo, DeploymentManager, DeploymentState, Queued,
};
use crate::error::{Error, Result};
use crate::persistence::Persistence;

use axum::body::Body;
use axum::extract::ws;
use axum::extract::{ws::WebSocket, BodyStream, Extension, Path};
use axum::routing::{get, Router};
use axum::Json;

use futures::TryStreamExt;

pub fn make_router(
    persistence: Persistence,
    deployment_manager: DeploymentManager,
    build_logs_manager: BuildLogsManager,
) -> Router<Body> {
    Router::new()
        .route("/services", get(list_services))
        .route(
            "/services/:name",
            get(get_service).post(post_service).delete(delete_service),
        )
        .route(
            "/services/:name/build-logs-subscribe",
            get(get_build_logs_subscribe),
        )
        .route("/services/:name/build-logs", get(get_build_logs))
        .layer(Extension(persistence))
        .layer(Extension(deployment_manager))
        .layer(Extension(build_logs_manager))
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
    Extension(build_logs_manager): Extension<BuildLogsManager>,
    Path(name): Path<String>,
    stream: BodyStream,
) -> Result<Json<DeploymentInfo>> {
    let build_log_writer = build_logs_manager.for_deployment(name.clone()).await;

    let queued = Queued {
        name,
        state: DeploymentState::Queued,
        data_stream: Box::pin(stream.map_err(Error::Streaming)),
        build_log_writer,
    };
    let info = DeploymentInfo::from(&queued);

    persistence.update_deployment(&queued).await?;
    deployment_manager.queue_push(queued).await;

    Ok(Json(info))
}

async fn delete_service(
    Extension(persistence): Extension<Persistence>,
    Extension(deployment_manager): Extension<DeploymentManager>,
    Extension(build_logs_manager): Extension<BuildLogsManager>,
    Path(name): Path<String>,
) -> Result<Json<Option<DeploymentInfo>>> {
    let old_info = persistence.delete_deployment(&name).await?;
    build_logs_manager.delete_deployment(&name).await;
    deployment_manager.kill(name).await;

    Ok(Json(old_info))
}

async fn get_build_logs(
    Extension(build_logs_manager): Extension<BuildLogsManager>,
    Path(name): Path<String>,
) -> Json<Option<Vec<String>>> {
    Json(build_logs_manager.get_logs_so_far(&name).await)
}

async fn get_build_logs_subscribe(
    Extension(build_logs_manager): Extension<BuildLogsManager>,
    Path(name): Path<String>,
    ws_upgrade: ws::WebSocketUpgrade,
) -> axum::response::Response {
    let log_recv = build_logs_manager.take_receiver(&name).await;

    ws_upgrade.on_upgrade(move |s| websocket_handler(s, log_recv))
}

async fn websocket_handler(mut s: WebSocket, log_recv: Option<BuildLogReceiver>) {
    if let Some(mut log_recv) = log_recv {
        while let Ok(msg) = log_recv.recv().await {
            let sent = s.send(ws::Message::Text(msg)).await;

            // Client disconnected?
            if sent.is_err() {
                return;
            }
        }
    }
    let _ = s.close().await;
}
