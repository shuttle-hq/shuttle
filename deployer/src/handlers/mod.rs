mod error;

use axum::body::{Body, BoxBody};
use axum::extract::ws::{self, WebSocket};
use axum::extract::{Extension, Path, Query};
use axum::http::{Request, Response};
use axum::routing::{get, Router};
use axum::{extract::BodyStream, Json};
use bytes::BufMut;
use chrono::{TimeZone, Utc};
use fqdn::FQDN;
use futures::StreamExt;
use shuttle_common::models::secret;
use shuttle_common::LogItem;
use tower_http::auth::RequireAuthorizationLayer;
use tower_http::trace::TraceLayer;
use tracing::{debug, debug_span, error, field, trace, Span};
use uuid::Uuid;

use crate::deployment::{DeploymentManager, Queued};
use crate::persistence::{Deployment, Log, Persistence, SecretGetter, State};

use std::collections::HashMap;
use std::time::Duration;

pub use {self::error::Error, self::error::Result};

pub fn make_router(
    persistence: Persistence,
    deployment_manager: DeploymentManager,
    proxy_fqdn: FQDN,
    admin_secret: String,
) -> Router<Body> {
    Router::new()
        .route("/projects/:project_name/services", get(list_services))
        .route(
            "/projects/:project_name/services/:service_name",
            get(get_service).post(post_service).delete(delete_service),
        )
        .route("/projects/:project_name/services/:service_name/summary", get(get_service_summary))
        .route(
            "/projects/:project_name/deployments/:deployment_id",
            get(get_deployment).delete(delete_deployment),
        )
        .route(
            "/projects/:project_name/ws/deployments/:deployment_id/logs",
            get(get_logs_subscribe),
        )
        .route("/projects/:project_name/deployments/:deployment_id/logs", get(get_logs))
        .route(
            "/projects/:project_name/secrets/:service_name",
            get(get_secrets),
        )
        .layer(Extension(persistence))
        .layer(Extension(deployment_manager))
        .layer(Extension(proxy_fqdn))
        .layer(RequireAuthorizationLayer::bearer(&admin_secret))
        // This route should be below the auth bearer since it does not need authentication
        .route("/projects/:project_name/status", get(get_status))
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(|request: &Request<Body>| {
                    debug_span!("request", http.uri = %request.uri(), http.method = %request.method(), http.status_code = field::Empty, api_key = field::Empty)
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
) -> Result<Json<Vec<shuttle_common::models::service::Response>>> {
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
    Path((_project_name, service_name)): Path<(String, String)>,
) -> Result<Json<shuttle_common::models::service::Detailed>> {
    if let Some(service) = persistence.get_service_by_name(&service_name).await? {
        let deployments = persistence
            .get_deployments(&service.id)
            .await?
            .into_iter()
            .map(Into::into)
            .collect();
        let resources = persistence
            .get_service_resources(&service.id)
            .await?
            .into_iter()
            .map(Into::into)
            .collect();
        let secrets = persistence
            .get_secrets(&service.id)
            .await?
            .into_iter()
            .map(Into::into)
            .collect();

        let response = shuttle_common::models::service::Detailed {
            name: service.name,
            deployments,
            resources,
            secrets,
        };

        Ok(Json(response))
    } else {
        Err(Error::NotFound)
    }
}

async fn get_service_summary(
    Extension(persistence): Extension<Persistence>,
    Extension(proxy_fqdn): Extension<FQDN>,
    Path((project_name, service_name)): Path<(String, String)>,
) -> Result<Json<shuttle_common::models::service::Summary>> {
    if let Some(service) = persistence.get_service_by_name(&service_name).await? {
        let deployment = persistence
            .get_active_deployment(&service.id)
            .await?
            .map(Into::into);
        let resources = persistence
            .get_service_resources(&service.id)
            .await?
            .into_iter()
            .map(Into::into)
            .collect();

        let response = shuttle_common::models::service::Summary {
            uri: format!("https://{}.{proxy_fqdn}", project_name),
            name: service.name,
            deployment,
            resources,
        };

        Ok(Json(response))
    } else {
        Err(Error::NotFound)
    }
}

async fn post_service(
    Extension(persistence): Extension<Persistence>,
    Extension(deployment_manager): Extension<DeploymentManager>,
    Path((_project_name, service_name)): Path<(String, String)>,
    Query(params): Query<HashMap<String, String>>,
    mut stream: BodyStream,
) -> Result<Json<shuttle_common::models::deployment::Response>> {
    let service = persistence.get_or_create_service(&service_name).await?;
    let id = Uuid::new_v4();

    let deployment = Deployment {
        id,
        service_id: service.id,
        state: State::Queued,
        last_update: Utc::now(),
        address: None,
    };

    let mut data = Vec::new();
    while let Some(buf) = stream.next().await {
        let buf = buf?;
        debug!("Received {} bytes", buf.len());
        data.put(buf);
    }
    debug!("Received a total of {} bytes", data.len());

    persistence.insert_deployment(deployment.clone()).await?;

    let queued = Queued {
        id,
        service_name: service.name,
        service_id: service.id,
        data,
        will_run_tests: !params.contains_key("no-test"),
        tracing_context: Default::default(),
    };

    deployment_manager.queue_push(queued).await;

    Ok(Json(deployment.into()))
}

async fn delete_service(
    Extension(persistence): Extension<Persistence>,
    Extension(deployment_manager): Extension<DeploymentManager>,
    Path((_project_name, service_name)): Path<(String, String)>,
) -> Result<Json<shuttle_common::models::service::Detailed>> {
    if let Some(service) = persistence.get_service_by_name(&service_name).await? {
        let old_deployments = persistence
            .delete_deployments_by_service_id(&service.id)
            .await?;

        for deployment in old_deployments.iter() {
            deployment_manager.kill(deployment.id).await;
        }

        let resources = persistence
            .get_service_resources(&service.id)
            .await?
            .into_iter()
            .map(Into::into)
            .collect();
        let secrets = persistence
            .get_secrets(&service.id)
            .await?
            .into_iter()
            .map(Into::into)
            .collect();

        persistence.delete_service(&service.id).await?;

        let response = shuttle_common::models::service::Detailed {
            name: service.name,
            deployments: old_deployments.into_iter().map(Into::into).collect(),
            resources,
            secrets,
        };

        Ok(Json(response))
    } else {
        Err(Error::NotFound)
    }
}

async fn get_deployment(
    Extension(persistence): Extension<Persistence>,
    Path((_project_name, deployment_id)): Path<(String, Uuid)>,
) -> Result<Json<shuttle_common::models::deployment::Response>> {
    if let Some(deployment) = persistence.get_deployment(&deployment_id).await? {
        Ok(Json(deployment.into()))
    } else {
        Err(Error::NotFound)
    }
}

async fn delete_deployment(
    Extension(deployment_manager): Extension<DeploymentManager>,
    Extension(persistence): Extension<Persistence>,
    Path((_project_name, deployment_id)): Path<(String, Uuid)>,
) -> Result<Json<shuttle_common::models::deployment::Response>> {
    if let Some(deployment) = persistence.get_deployment(&deployment_id).await? {
        deployment_manager.kill(deployment.id).await;

        Ok(Json(deployment.into()))
    } else {
        Err(Error::NotFound)
    }
}

async fn get_logs(
    Extension(persistence): Extension<Persistence>,
    Path((_project_name, deployment_id)): Path<(String, Uuid)>,
) -> Result<Json<Vec<LogItem>>> {
    if let Some(deployment) = persistence.get_deployment(&deployment_id).await? {
        Ok(Json(
            persistence
                .get_deployment_logs(&deployment.id)
                .await?
                .into_iter()
                .filter_map(Into::into)
                .collect(),
        ))
    } else {
        Err(Error::NotFound)
    }
}

async fn get_logs_subscribe(
    Extension(persistence): Extension<Persistence>,
    Path((_project_name, deployment_id)): Path<(String, Uuid)>,
    ws_upgrade: ws::WebSocketUpgrade,
) -> axum::response::Response {
    ws_upgrade.on_upgrade(move |s| logs_websocket_handler(s, persistence, deployment_id))
}

async fn logs_websocket_handler(mut s: WebSocket, persistence: Persistence, id: Uuid) {
    let mut log_recv = persistence.get_log_subscriber();
    let backlog = match persistence.get_deployment_logs(&id).await {
        Ok(backlog) => backlog,
        Err(error) => {
            error!(
                error = &error as &dyn std::error::Error,
                "failed to get backlog of logs"
            );

            let _ = s
                .send(ws::Message::Text("failed to get logs".to_string()))
                .await;
            let _ = s.close().await;
            return;
        }
    };
    let mut last_timestamp = Utc.timestamp(0, 0);

    for log in backlog.into_iter() {
        last_timestamp = log.timestamp;
        if let Some(log_item) = Option::<LogItem>::from(log) {
            let msg = serde_json::to_string(&log_item).expect("to convert log item to json");
            let sent = s.send(ws::Message::Text(msg)).await;

            // Client disconnected?
            if sent.is_err() {
                return;
            }
        }
    }

    while let Ok(log) = log_recv.recv().await {
        trace!(?log, "received log from broadcast channel");

        if log.id == id && log.timestamp > last_timestamp {
            if let Some(log_item) = Option::<LogItem>::from(Log::from(log)) {
                let msg = serde_json::to_string(&log_item).expect("to convert log item to json");
                let sent = s.send(ws::Message::Text(msg)).await;

                // Client disconnected?
                if sent.is_err() {
                    return;
                }
            }
        }
    }

    let _ = s.close().await;
}

async fn get_secrets(
    Extension(persistence): Extension<Persistence>,
    Path((_project_name, service_name)): Path<(String, String)>,
) -> Result<Json<Vec<secret::Response>>> {
    if let Some(service) = persistence.get_service_by_name(&service_name).await? {
        let keys = persistence
            .get_secrets(&service.id)
            .await?
            .into_iter()
            .map(Into::into)
            .collect();

        Ok(Json(keys))
    } else {
        Err(Error::NotFound)
    }
}

async fn get_status() -> String {
    "Ok".to_string()
}
