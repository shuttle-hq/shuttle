use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{extract::Form, Router};
use shuttle_runtime::tracing::info;

use crate::{AppState, RawJob};

pub fn build_router(app_state: Arc<AppState>) -> Router {
    Router::new()
        .route("/", get(hello_world))
        .route("/set-schedule", post(set_schedule))
        .with_state(app_state)
}

pub async fn hello_world() -> impl IntoResponse {
    (StatusCode::OK, "Hello world!").into_response()
}

pub async fn set_schedule(
    State(state): State<Arc<AppState>>,
    Form(job): Form<RawJob>,
) -> impl IntoResponse {
    info!("Accepted new job: {:?}", job);

    state.sender.send(job).await.unwrap();

    StatusCode::OK
}
