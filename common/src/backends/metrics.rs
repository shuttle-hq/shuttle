use std::{collections::HashMap, convert::Infallible};

use async_trait::async_trait;
use axum::extract::{FromRequestParts, Path};
use axum::http::request::Parts;
use tracing::Span;

/// Used to record a bunch of metrics info
/// The tracing layer on the server should record a `request.params.<param>` field for each parameter
/// that should be recorded
pub struct Metrics;

#[async_trait]
impl<S> FromRequestParts<S> for Metrics
where
    S: Send + Sync,
{
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        // Get path parameters if they exist
        let Path(path): Path<HashMap<String, String>> =
            match Path::from_request_parts(parts, state).await {
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
