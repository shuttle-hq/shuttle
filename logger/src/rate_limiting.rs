use std::net::IpAddr;

use tonic::{
    metadata::{KeyAndValueRef, MetadataMap},
    transport::server::TcpConnectInfo,
    Status,
};
use tower::BoxError;
use tower_governor::{key_extractor::KeyExtractor, GovernorError};

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
                Status::unavailable("unable to extract client address")
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
