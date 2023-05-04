use axum::{
    extract::{BodyStream, Path, Query},
    Extension, Json,
};
use bytes::BufMut;
use futures::StreamExt;
use shuttle_common::claims::Claim;
use std::collections::HashMap;
use tracing::{debug, error, instrument};

use super::error::Result;

#[instrument(skip_all, fields(%project_name))]
#[utoipa::path(
    post,
    path = "/deploy/{project_name}",
    responses(
        (status = 200, description = "Deploys a project by receiving an associated project archive.", content_type = "application/json", body = String),
        (status = 500, description = "Error while receiving byte stream.", body = String),
    ),
    params(
        ("project_name" = String, Path, description = "Name of the project that owns the service.")
    )
)]
pub async fn deploy_project(
    Extension(_claim): Extension<Claim>,
    Path(project_name): Path<String>,
    Query(_params): Query<HashMap<String, String>>,
    mut stream: BodyStream,
) -> Result<Json<String>> {
    let mut data = Vec::new();
    debug!("Starting byte stream reading");
    while let Some(buf) = stream.next().await {
        let buf = buf.map_err(|err| error!("{:?}", err)).unwrap();
        debug!("Received {} bytes", buf.len());
        data.put(buf);
    }
    debug!("Received a total of {} bytes", data.len());

    Ok(Json(format!("Received a total of {} bytes", data.len())))
}
