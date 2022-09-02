mod admin;
mod deployment;
mod service;
mod user;

use axum::body::{Body, BoxBody};
use axum::extract::ws::{self, WebSocket};
use axum::extract::{Extension, Path, Query};
use axum::http::{Request, Response};
use axum::routing::{get, post, Router};
use axum::{extract::BodyStream, Json};
use chrono::{TimeZone, Utc};
use fqdn::FQDN;
use futures::TryStreamExt;
use shuttle_common::{log, secret, LogItem};
use tower_http::trace::TraceLayer;
use tracing::{debug, debug_span, error, field, Span};
use uuid::Uuid;

use crate::deployment::{DeploymentManager, Queued};
use crate::error::{Error, Result};
use crate::persistence::{self, Deployment, Log, Persistence, SecretGetter, State};

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use self::admin::{AdminGuard, AdminSecret};
pub use self::deployment::DeploymentAuthorizer;
use self::deployment::DeploymentGuard;
pub use self::service::ServiceAuthorizer;
use self::service::ServiceGuard;
use self::user::UserGuard;
pub use self::user::UserValidator;

pub fn make_router(
    persistence: Persistence,
    deployment_manager: DeploymentManager,
    proxy_fqdn: FQDN,
    admin_secret: String,
) -> Router<Body> {
    Router::new()
        .route("/users/:name", post(get_or_create_user))
        .route("/services", get(list_services))
        .route(
            "/services/:name",
            get(get_service).post(post_service).delete(delete_service),
        )
        .route("/services/:name/summary", get(get_service_summary))
        .route(
            "/deployments/:id",
            get(get_deployment).delete(delete_deployment),
        )
        .route(
            "/ws/deployments/:id/logs/build",
            get(get_build_logs_subscribe),
        )
        .route("/deployments/:id/logs/build", get(get_build_logs))
        .route(
            "/ws/deployments/:id/logs/runtime",
            get(get_runtime_logs_subscribe),
        )
        .route("/deployments/:id/logs/runtime", get(get_runtime_logs))
        .route("/version", get(get_version))
        .route(
            "/secrets/:service_name",
            get(get_secrets),
        )
        .layer(Extension(persistence.clone()))
        .layer(Extension(deployment_manager))
        .layer(Extension(proxy_fqdn))
        .layer(Extension(AdminSecret(admin_secret)))
        .layer(&Extension::<Arc<dyn UserValidator>>(Arc::new(persistence.clone())))
        .layer(&Extension::<Arc<dyn ServiceAuthorizer>>(Arc::new(persistence.clone())))
        .layer(&Extension::<Arc<dyn DeploymentAuthorizer>>(Arc::new(persistence)))
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
) -> Result<Json<Vec<shuttle_common::service::Response>>> {
    let services = persistence
        .get_all_services()
        .await?
        .into_iter()
        .map(Into::into)
        .collect();

    Ok(Json(services))
}

async fn get_service(
    Extension(persistence): Extension<Persistence>,
    service_guard: ServiceGuard,
) -> Result<Json<shuttle_common::service::Detailed>> {
    let deployments = persistence
        .get_deployments(&service_guard.id)
        .await?
        .into_iter()
        .map(Into::into)
        .collect();
    let resources = persistence
        .get_service_resources(&service_guard.id)
        .await?
        .into_iter()
        .map(Into::into)
        .collect();
    let secrets = persistence
        .get_secrets(&service_guard.id)
        .await?
        .into_iter()
        .map(Into::into)
        .collect();

    let response = shuttle_common::service::Detailed {
        name: service_guard.name,
        deployments,
        resources,
        secrets,
    };

    Ok(Json(response))
}

async fn get_service_summary(
    Extension(persistence): Extension<Persistence>,
    Extension(proxy_fqdn): Extension<FQDN>,
    service_guard: ServiceGuard,
) -> Result<Json<shuttle_common::service::Summary>> {
    let deployment = persistence
        .get_active_deployment(&service_guard.id)
        .await?
        .map(Into::into);
    let resources = persistence
        .get_service_resources(&service_guard.id)
        .await?
        .into_iter()
        .map(Into::into)
        .collect();

    let response = shuttle_common::service::Summary {
        uri: format!("https://{}.{proxy_fqdn}", service_guard.name),
        name: service_guard.name,
        deployment,
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
    user: UserGuard,
) -> Result<Json<shuttle_common::deployment::Response>> {
    let service = persistence
        .get_or_create_service(&name, &user.api_key)
        .await?;
    let id = Uuid::new_v4();

    let deployment = Deployment {
        id,
        service_id: service.id,
        state: State::Queued,
        last_update: Utc::now(),
    };

    persistence.insert_deployment(deployment.clone()).await?;

    let queued = Queued {
        id,
        service_name: service.name,
        service_id: service.id,
        data_stream: Box::pin(stream.map_err(Error::Streaming)),
        will_run_tests: !params.contains_key("no-test"),
    };

    deployment_manager.queue_push(queued).await;

    Ok(Json(deployment.into()))
}

async fn delete_service(
    Extension(persistence): Extension<Persistence>,
    Extension(deployment_manager): Extension<DeploymentManager>,
    service_guard: ServiceGuard,
) -> Result<Json<shuttle_common::service::Detailed>> {
    persistence.delete_service(&service_guard.id).await?;
    let old_deployments = persistence
        .delete_deployments_by_service_id(&service_guard.id)
        .await?;

    for deployment in old_deployments.iter() {
        deployment_manager.kill(deployment.id).await;
    }

    let resources = persistence
        .get_service_resources(&service_guard.id)
        .await?
        .into_iter()
        .map(Into::into)
        .collect();
    let secrets = persistence
        .get_secrets(&service_guard.id)
        .await?
        .into_iter()
        .map(Into::into)
        .collect();

    let response = shuttle_common::service::Detailed {
        name: service_guard.name,
        deployments: old_deployments.into_iter().map(Into::into).collect(),
        resources,
        secrets,
    };

    Ok(Json(response))
}

async fn get_deployment(
    deployment_guard: DeploymentGuard,
) -> Result<Json<shuttle_common::deployment::Response>> {
    Ok(Json(deployment_guard.into()))
}

async fn delete_deployment(
    Extension(deployment_manager): Extension<DeploymentManager>,
    deployment_guard: DeploymentGuard,
) -> Result<Json<shuttle_common::deployment::Response>> {
    deployment_manager.kill(deployment_guard.id).await;

    Ok(Json(deployment_guard.into()))
}

async fn get_build_logs(
    Extension(persistence): Extension<Persistence>,
    deployment_guard: DeploymentGuard,
) -> Result<Json<Vec<log::BuildLogStream>>> {
    Ok(Json(
        persistence
            .get_deployment_logs(&deployment_guard.id)
            .await?
            .into_iter()
            .filter(|log| matches!(log.state, State::Building))
            .filter_map(Log::into_build_log_stream)
            .collect(),
    ))
}

async fn get_runtime_logs(
    Extension(persistence): Extension<Persistence>,
    deployment_guard: DeploymentGuard,
) -> Result<Json<Vec<LogItem>>> {
    Ok(Json(
        persistence
            .get_deployment_logs(&deployment_guard.id)
            .await?
            .into_iter()
            .filter(|log| matches!(log.state, State::Running))
            .map(Into::into)
            .collect(),
    ))
}

async fn get_build_logs_subscribe(
    Extension(persistence): Extension<Persistence>,
    deployment_guard: DeploymentGuard,
    ws_upgrade: ws::WebSocketUpgrade,
) -> axum::response::Response {
    ws_upgrade
        .on_upgrade(move |s| build_logs_websocket_handler(s, persistence, deployment_guard.id))
}

async fn build_logs_websocket_handler(mut s: WebSocket, persistence: Persistence, id: Uuid) {
    let mut log_recv = persistence.get_log_subscriber();
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

    for log in backlog.into_iter().filter_map(Log::into_build_log_stream) {
        match (log.state, log.message) {
            (shuttle_common::deployment::State::Building, Some(msg)) => {
                let sent = s.send(ws::Message::Text(msg)).await;
                last_timestamp = log.timestamp;

                // Client disconnected?
                if sent.is_err() {
                    return;
                }
            }
            (shuttle_common::deployment::State::Building, None) => {}
            (shuttle_common::deployment::State::Queued, _)
            | (shuttle_common::deployment::State::Built, _) => {}
            _ => {
                debug!("closing channel after reaching more than just build logs");
                let _ = s.close().await;
                return;
            }
        }
    }

    while let Ok(log) = log_recv.recv().await {
        if log.id == id && log.timestamp > last_timestamp {
            if let Some(log) = persistence::Log::from(log).into_build_log_stream() {
                match (log.state, log.message) {
                    (shuttle_common::deployment::State::Building, Some(msg)) => {
                        let sent = s.send(ws::Message::Text(msg)).await;

                        // Client disconnected?
                        if sent.is_err() {
                            return;
                        }
                    }
                    (shuttle_common::deployment::State::Queued, _)
                    | (shuttle_common::deployment::State::Built, _) => {}
                    (shuttle_common::deployment::State::Building, None) => {}
                    _ => break,
                }
            }
        }
    }

    debug!("build log channel has reached its end");
    let _ = s.close().await;
}

async fn get_runtime_logs_subscribe(
    Extension(persistence): Extension<Persistence>,
    deployment_guard: DeploymentGuard,
    ws_upgrade: ws::WebSocketUpgrade,
) -> axum::response::Response {
    ws_upgrade
        .on_upgrade(move |s| runtime_logs_websocket_handler(s, persistence, deployment_guard.id))
}

async fn runtime_logs_websocket_handler(mut s: WebSocket, persistence: Persistence, id: Uuid) {
    let mut log_recv = persistence.get_log_subscriber();
    let backlog = match persistence.get_deployment_logs(&id).await {
        Ok(backlog) => backlog,
        Err(error) => {
            error!(
                error = &error as &dyn std::error::Error,
                "failed to get backlog runtime logs"
            );

            let _ = s
                .send(ws::Message::Text("failed to get runtime logs".to_string()))
                .await;
            let _ = s.close().await;
            return;
        }
    };
    let mut last_timestamp = Utc.timestamp(0, 0);
    let mut last_state = State::Queued;

    for log in backlog.into_iter() {
        last_state = log.state;
        if log.state == State::Running {
            last_timestamp = log.timestamp;
            let msg =
                serde_json::to_string(&LogItem::from(log)).expect("to convert log item to json");
            let sent = s.send(ws::Message::Text(msg)).await;

            // Client disconnected?
            if sent.is_err() {
                return;
            }
        }
    }

    if last_state != State::Running {
        debug!("closing channel after reaching more than just running logs");
        let _ = s.close().await;

        return;
    }

    while let Ok(log) = log_recv.recv().await {
        if log.id == id && log.timestamp > last_timestamp {
            match log.state {
                State::Running => {
                    let msg = serde_json::to_string(&LogItem::from(log))
                        .expect("to convert log item to json");
                    let sent = s.send(ws::Message::Text(msg)).await;

                    // Client disconnected?
                    if sent.is_err() {
                        return;
                    }
                }
                State::Queued | State::Building | State::Built => {}
                _ => break,
            }
        }
    }

    let _ = s.close().await;
}

async fn get_version() -> String {
    shuttle_service::VERSION.to_string()
}

async fn get_secrets(
    Extension(persistence): Extension<Persistence>,
    service_guard: ServiceGuard,
) -> Result<Json<Vec<secret::Response>>> {
    let keys = persistence
        .get_secrets(&service_guard.id)
        .await?
        .into_iter()
        .map(Into::into)
        .collect();

    Ok(Json(keys))
}

// TODO: move to gateway
async fn get_or_create_user(
    Extension(persistence): Extension<Persistence>,
    Path(api_key): Path<String>,
    _: AdminGuard,
) -> Result<String> {
    persistence
        .get_or_create_user(&api_key)
        .await
        .map(|user| user.api_key)
        .map_err(Error::from)
}
