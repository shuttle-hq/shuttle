use std::net::IpAddr;

use tonic::{
    metadata::{KeyAndValueRef, MetadataMap},
    transport::server::TcpConnectInfo,
    Status,
};
use tower::BoxError;
use tower_governor::{key_extractor::KeyExtractor, GovernorError};

/// The interval at which the rate limiter refreshes one slot in milliseconds.
pub const REFRESH_INTERVAL: u64 = 500;
/// The quota of requests that can be received before rate limiting is applied.
pub const BURST_SIZE: u32 = 6;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TonicPeerIpKeyExtractor;

impl KeyExtractor for TonicPeerIpKeyExtractor {
    type Key = IpAddr;

    fn name(&self) -> &'static str {
        "peer IP"
    }

    fn extract<T>(&self, req: &http::Request<T>) -> Result<Self::Key, GovernorError> {
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

/// Convert errors from the Governor rate limiter layer to tonic statuses.
pub fn tonic_error(e: BoxError) -> tonic::Status {
    if let Some(error) = e.downcast_ref::<GovernorError>() {
        match error.to_owned() {
            GovernorError::TooManyRequests { wait_time, headers } => {
                let mut response = Status::unavailable(format!(
                    "received too many requests, wait for {wait_time}ms"
                ));

                // Add rate limiting headers: x-ratelimit-remaining, x-ratelimit-after, x-ratelimit-limit.
                if let Some(headers) = headers {
                    let metadata = MetadataMap::from_headers(headers);

                    for header in metadata.iter() {
                        if let KeyAndValueRef::Ascii(key, value) = header {
                            response.metadata_mut().insert(key, value.clone());
                        }
                    }
                }

                response
            }
            GovernorError::UnableToExtractKey => {
                Status::unavailable("unable to extract peer address")
            }
            GovernorError::Other { headers, .. } => {
                let mut response = Status::internal("unexpected error in rate limiter");

                if let Some(headers) = headers {
                    let metadata = MetadataMap::from_headers(headers);

                    for header in metadata.iter() {
                        if let KeyAndValueRef::Ascii(key, value) = header {
                            response.metadata_mut().insert(key, value.clone());
                        }
                    }
                }

                response
            }
        }
    } else {
        Status::internal("unexpected error in rate limiter")
    }
}
