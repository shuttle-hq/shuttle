use std::time::Duration;

use bytes::Bytes;
use headers::{Authorization, HeaderMapExt};
use http::{HeaderMap, HeaderValue, Method, StatusCode, Uri};
use opentelemetry::global;
use opentelemetry_http::HeaderInjector;
use reqwest::{Client, ClientBuilder, Response};
use serde::{de::DeserializeOwned, Serialize};
use thiserror::Error;
use tracing::{trace, Span};
use tracing_opentelemetry::OpenTelemetrySpanExt;

pub mod gateway;
pub mod permit;
mod resource_recorder;

pub use gateway::ProjectsDal;
pub use permit::PermissionsDal;
pub use resource_recorder::ResourceDal;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Reqwest error: {0}")]
    Reqwest(#[from] reqwest::Error),
    #[error("Serde JSON error: {0}")]
    SerdeJson(#[from] serde_json::Error),
    #[error("Request did not return correctly. Got status code: {0}")]
    RequestError(StatusCode),
    #[error("GRpc request did not return correctly. Got status code: {0}")]
    GrpcError(#[from] tonic::Status),
}

/// `reqwest` wrapper to make requests to other services easy
#[derive(Clone)]
pub struct ServicesApiClient {
    client: Client,
    base: Uri,
}

impl ServicesApiClient {
    pub fn builder() -> ClientBuilder {
        Client::builder().timeout(Duration::from_secs(60))
    }

    pub fn new(base: Uri) -> Self {
        Self {
            client: Self::builder().build().unwrap(),
            base,
        }
    }

    pub fn new_with_bearer(base: Uri, token: &str) -> Self {
        Self {
            client: Self::builder()
                .default_headers(header_map_with_bearer(token))
                .build()
                .unwrap(),
            base,
        }
    }

    pub async fn get<T: DeserializeOwned>(
        &self,
        path: &str,
        headers: Option<HeaderMap<HeaderValue>>,
    ) -> Result<T, Error> {
        self.request(Method::GET, path, None::<()>, headers).await
    }

    pub async fn post<B: Serialize, T: DeserializeOwned>(
        &self,
        path: &str,
        body: B,
        headers: Option<HeaderMap<HeaderValue>>,
    ) -> Result<T, Error> {
        self.request(Method::POST, path, Some(body), headers).await
    }

    pub async fn delete<B: Serialize, T: DeserializeOwned>(
        &self,
        path: &str,
        body: B,
        headers: Option<HeaderMap<HeaderValue>>,
    ) -> Result<T, Error> {
        self.request(Method::DELETE, path, Some(body), headers)
            .await
    }

    pub async fn request<B: Serialize, T: DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        body: Option<B>,
        headers: Option<HeaderMap<HeaderValue>>,
    ) -> Result<T, Error> {
        Ok(self
            .request_raw(method, path, body, headers)
            .await?
            .json()
            .await?)
    }

    pub async fn request_bytes<B: Serialize>(
        &self,
        method: Method,
        path: &str,
        body: Option<B>,
        headers: Option<HeaderMap<HeaderValue>>,
    ) -> Result<Bytes, Error> {
        Ok(self
            .request_raw(method, path, body, headers)
            .await?
            .bytes()
            .await?)
    }

    // can be used for explicit HEAD requests (ignores body)
    pub async fn request_raw<B: Serialize>(
        &self,
        method: Method,
        path: &str,
        body: Option<B>,
        headers: Option<HeaderMap<HeaderValue>>,
    ) -> Result<Response, Error> {
        let uri = format!("{}{path}", self.base);
        trace!(uri, "calling inner service");

        let mut h = headers.unwrap_or_default();
        let cx = Span::current().context();
        global::get_text_map_propagator(|propagator| {
            propagator.inject_context(&cx, &mut HeaderInjector(&mut h))
        });
        let req = self.client.request(method, uri).headers(h);
        let req = if let Some(body) = body {
            req.json(&body)
        } else {
            req
        };

        let resp = req.send().await?;
        trace!(response = ?resp, "service response");

        if !resp.status().is_success() {
            return Err(Error::RequestError(resp.status()));
        }

        Ok(resp)
    }
}

pub fn header_map_with_bearer(token: &str) -> HeaderMap {
    let mut h = HeaderMap::new();
    h.typed_insert(Authorization::bearer(token).expect("valid token"));
    h
}

#[cfg(test)]
mod tests {
    use http::StatusCode;

    use crate::models;
    use crate::test_utils::get_mocked_gateway_server;

    use super::{Error, ServicesApiClient};

    // Make sure we handle any unexpected responses correctly
    #[tokio::test]
    async fn api_errors() {
        let server = get_mocked_gateway_server().await;

        let client = ServicesApiClient::new(server.uri().parse().unwrap());

        let err = client
            .get::<Vec<models::project::Response>>("projects", None)
            .await
            .unwrap_err();

        assert!(matches!(err, Error::RequestError(StatusCode::UNAUTHORIZED)));
    }
}
