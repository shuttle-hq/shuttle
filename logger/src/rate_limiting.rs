use std::net::IpAddr;

use axum::http::{request, Response};
use tonic::{body::BoxBody, transport::server::TcpConnectInfo, Status};
use tower::BoxError;
use tower_governor::{key_extractor::KeyExtractor, GovernorError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TonicPeerIpKeyExtractor;

impl KeyExtractor for TonicPeerIpKeyExtractor {
    type Key = IpAddr;

    fn name(&self) -> &'static str {
        "peer IP"
    }

    fn extract<T>(&self, req: &request::Request<T>) -> Result<Self::Key, GovernorError> {
        req.extensions()
            .get::<TcpConnectInfo>()
            .and_then(|info| info.remote_addr())
            .map(|addr| addr.ip())
            .ok_or(GovernorError::UnableToExtractKey)
    }

    fn key_name(&self, key: &Self::Key) -> Option<String> {
        Some(key.to_string())
    }
}

/// Convert errors from the Governor rate limiter layer to tonic statuses. The errors are
/// captured by an axum error handling layer, so we convert the tonic statuses to http for
/// compatibility with that layer.
pub fn tonic_error(e: BoxError) -> Response<BoxBody> {
    if e.is::<GovernorError>() {
        // It shouldn't be possible for this to panic, since we already know it's a GovernorError
        let error = e.downcast_ref::<GovernorError>().unwrap().to_owned();
        match error {
            GovernorError::TooManyRequests { wait_time, headers } => {
                // TODO: after upgrading tonic, use tonic types trait extensions to enrich status,
                // see example: https://github.com/hyperium/tonic/blob/master/examples/src/richer-error/server.rs.
                // We can for example add wait time as: https://docs.rs/tonic-types/latest/tonic_types/struct.RetryInfo.html

                let mut response = Status::unavailable(format!(
                    "received too many requests, wait for {wait_time}s"
                ))
                .to_http();

                // Add rate limiting headers: x-ratelimit-remaining, x-ratelimit-after, x-ratelimit-limit.
                if let Some(headers) = headers {
                    response.headers_mut().extend(headers);
                }

                response
            }
            GovernorError::UnableToExtractKey => {
                Status::unavailable("unable to extract client address").to_http()
            }
            GovernorError::Other { headers, .. } => {
                let mut response = Status::internal("unexpected error in rate limiter").to_http();

                if let Some(headers) = headers {
                    response.headers_mut().extend(headers);
                }

                response
            }
        }
    } else {
        Status::internal("unexpected error in rate limiter").to_http()
    }
}
