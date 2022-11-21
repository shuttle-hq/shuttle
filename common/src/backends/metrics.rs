use std::{collections::HashMap, convert::Infallible};

use async_trait::async_trait;
use axum::extract::{FromRequest, Path, RequestParts};
use tracing::Span;

/// Used to record a bunch of metrics info
/// The tracing layer on the server should record a `request.params.<param>` field for each parameter
/// that should be recorded
pub struct Metrics;

#[async_trait]
impl<B> FromRequest<B> for Metrics
where
    B: Send,
{
    type Rejection = Infallible;

    async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
        // Get path parameters if they exist
        let Path(path): Path<HashMap<String, String>> = match req.extract().await {
            Ok(path) => path,
            Err(_) => return Ok(Metrics),
        };

        let span = Span::current();

        for (param, value) in path {
            span.record(format!("request.params.{param}").as_str(), value);
        }
        Ok(Metrics)
    }
}
