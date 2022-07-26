use axum::body::Body;
use axum::extract::ws::WebSocket;
use axum::extract::{ws, Extension, Path, Query};
use axum::routing::{get, Router};
use axum::{extract::BodyStream, Json};
use chrono::{TimeZone, Utc};
use futures::TryStreamExt;
use shuttle_common::BuildLog;
use tracing::error;

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
        .route(
            "/services/:name/build-logs-subscribe",
            get(get_build_logs_subscribe),
        )
        .route("/services/:name/build-logs", get(get_build_logs))
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

async fn get_build_logs(
    Extension(persistence): Extension<Persistence>,
    Path(name): Path<String>,
) -> Result<Json<Vec<BuildLog>>> {
    persistence.get_build_logs(&name).await.map(Json)
}

async fn get_build_logs_subscribe(
    Extension(persistence): Extension<Persistence>,
    Path(name): Path<String>,
    ws_upgrade: ws::WebSocketUpgrade,
) -> axum::response::Response {
    ws_upgrade.on_upgrade(move |s| websocket_handler(s, persistence, name))
}

async fn websocket_handler(mut s: WebSocket, persistence: Persistence, name: String) {
    let mut log_recv = persistence.get_build_log_subscriber();
    let backlog = match persistence.get_build_logs(&name).await {
        Ok(backlog) => backlog,
        Err(error) => {
            error!(
                error = &error as &dyn std::error::Error,
                "failed to get backlog build logs"
            );

            let _ = s
                .send(ws::Message::Text("failed to get build logs".to_string()))
                .await;
            let _ = s.close().await;
            return;
        }
    };
    let mut last_timestamp = Utc.timestamp(0, 0);

    for log in backlog {
        let sent = s.send(ws::Message::Text(log.message)).await;
        last_timestamp = log.timestamp;

        // Client disconnected?
        if sent.is_err() {
            return;
        }
    }

    while let Ok(msg) = log_recv.recv().await {
        if msg.name == name && msg.timestamp > last_timestamp {
            let sent = s.send(ws::Message::Text(msg.message)).await;

            // Client disconnected?
            if sent.is_err() {
                return;
            }
        }
    }

    let _ = s.close().await;
}
