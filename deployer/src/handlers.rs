use axum::body::{Body, BoxBody};
use axum::extract::ws::{self, WebSocket};
use axum::extract::{Extension, Path, Query};
use axum::http::{Request, Response};
use axum::routing::{get, Router};
use axum::{extract::BodyStream, Json};
use chrono::{TimeZone, Utc};
use futures::TryStreamExt;
use shuttle_common::{deployment, log, service};
use tower_http::trace::TraceLayer;
use tracing::{debug, debug_span, error, field, Span};
use uuid::Uuid;

use crate::deployment::{DeploymentManager, Log, Queued, State};
use crate::error::{Error, Result};
use crate::persistence::{Deployment, Persistence};

use std::collections::HashMap;
use std::time::Duration;

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
        .route(
            "/deployments/:id/build-logs-subscribe",
            get(get_build_logs_subscribe),
        )
        .route("/deployments/:id/build-logs", get(get_build_logs))
        .route("/version", get(get_version))
        .layer(Extension(persistence))
        .layer(Extension(deployment_manager))
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(|request: &Request<Body>| {
                    debug_span!("request", http.uri = %request.uri(), http.method = %request.method(), http.status_code = field::Empty)
                })
                .on_response(
                    |response: &Response<BoxBody>, latency: Duration, span: &Span| {
                        span.record("http.status_code", &response.status().as_u16());
                        debug!(latency = format_args!("{} ns", latency.as_nanos()), "finished processing request");
                    },
                ),
        )
}

async fn list_services(
    Extension(persistence): Extension<Persistence>,
) -> Result<Json<Vec<String>>> {
    persistence.get_all_services().await.map(Json)
}

async fn get_service(
    Extension(persistence): Extension<Persistence>,
    Path(name): Path<String>,
) -> Result<Json<service::Response>> {
    let deployments = persistence
        .get_deployments(&name)
        .await?
        .into_iter()
        .map(Into::into)
        .collect();
    let resources = persistence
        .get_service_resources(&name)
        .await?
        .into_iter()
        .map(Into::into)
        .collect();

    let response = service::Response {
        uri: format!("{name}.shuttleapp.rs"),
        name,
        deployments,
        resources,
    };

    Ok(Json(response))
}

async fn post_service(
    Extension(persistence): Extension<Persistence>,
    Extension(deployment_manager): Extension<DeploymentManager>,
    Path(name): Path<String>,
    Query(params): Query<HashMap<String, String>>,
    stream: BodyStream,
) -> Result<Json<deployment::Response>> {
    let id = Uuid::new_v4();

    let deployment = deployment::Response {
        id,
        name: name.clone(),
        state: deployment::State::Queued,
        last_update: Utc::now(),
    };

    persistence.insert_deployment(&deployment).await?;

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

async fn get_build_logs(
    Extension(persistence): Extension<Persistence>,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<log::StreamLog>>> {
    Ok(Json(
        persistence
            .get_deployment_logs(&id)
            .await?
            .into_iter()
            .filter(|log| matches!(log.state, State::Building))
            .filter_map(Log::into_stream_log)
            .collect(),
    ))
}

async fn get_build_logs_subscribe(
    Extension(persistence): Extension<Persistence>,
    Path(id): Path<Uuid>,
    ws_upgrade: ws::WebSocketUpgrade,
) -> axum::response::Response {
    ws_upgrade.on_upgrade(move |s| websocket_handler(s, persistence, id))
}

async fn websocket_handler(mut s: WebSocket, persistence: Persistence, id: Uuid) {
    let mut log_recv = persistence.get_stream_log_subscriber();
    let backlog = match persistence.get_deployment_logs(&id).await {
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

    for log in backlog.into_iter().filter_map(Log::into_stream_log) {
        match (log.state, log.message) {
            (deployment::State::Building, Some(msg)) => {
                let sent = s.send(ws::Message::Text(msg)).await;
                last_timestamp = log.timestamp;

                // Client disconnected?
                if sent.is_err() {
                    return;
                }
            }
            (deployment::State::Building, None) => {}
            (deployment::State::Queued, _) => {}
            _ => {
                debug!("closing channel after reaching more than just build logs");
                let _ = s.close().await;
                return;
            }
        }
    }

    while let Ok(log) = log_recv.recv().await {
        if log.id == id && log.timestamp > last_timestamp {
            match (log.state, log.message) {
                (deployment::State::Building, Some(msg)) => {
                    let sent = s.send(ws::Message::Text(msg)).await;

                    // Client disconnected?
                    if sent.is_err() {
                        return;
                    }
                }
                (deployment::State::Queued, _) => {}
                (deployment::State::Building, None) => {}
                _ => break,
            }
        }
    }

    let _ = s.close().await;
}

async fn get_version() -> String {
    shuttle_service::VERSION.to_string()
}
