use bytes::Bytes;
use headers::{ContentType, Header, HeaderMapExt};
use http::{Method, Request, StatusCode, Uri};
use hyper::{body, client::HttpConnector, Body, Client};
use opentelemetry::global;
use opentelemetry_http::HeaderInjector;
use serde::{de::DeserializeOwned, Serialize};
use thiserror::Error;
use tracing::{trace, Span};
use tracing_opentelemetry::OpenTelemetrySpanExt;

pub mod gateway;
mod resource_recorder;

pub use gateway::ProjectsDal;
pub use resource_recorder::ResourceDal;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Hyper error: {0}")]
    Hyper(#[from] hyper::Error),
    #[error("Serde JSON error: {0}")]
    SerdeJson(#[from] serde_json::Error),
    #[error("Hyper error: {0}")]
    Http(#[from] hyper::http::Error),
    #[error("Request did not return correctly. Got status code: {0}")]
    RequestError(StatusCode),
    #[error("GRpc request did not return correctly. Got status code: {0}")]
    GrpcError(#[from] tonic::Status),
}

/// `Hyper` wrapper to make request to RESTful Shuttle services easy
#[derive(Clone)]
pub struct ServicesApiClient {
    client: Client<HttpConnector>,
    base: Uri,
}

impl ServicesApiClient {
    fn new(base: Uri) -> Self {
        Self {
            client: Client::new(),
            base,
        }
    }

    pub async fn request<B: Serialize, T: DeserializeOwned, H: Header>(
        &self,
        method: Method,
        path: &str,
        body: Option<B>,
        extra_header: Option<H>,
    ) -> Result<T, Error> {
        let bytes = self.request_raw(method, path, body, extra_header).await?;
        let json = serde_json::from_slice(&bytes)?;

        Ok(json)
    }

    pub async fn request_raw<B: Serialize, H: Header>(
        &self,
        method: Method,
        path: &str,
        body: Option<B>,
        extra_header: Option<H>,
    ) -> Result<Bytes, Error> {
        let uri = format!("{}{path}", self.base);
        trace!(uri, "calling inner service");

        let mut req = Request::builder().method(method).uri(uri);
        let headers = req
            .headers_mut()
            .expect("new request to have mutable headers");
        if let Some(extra_header) = extra_header {
            headers.typed_insert(extra_header);
        }
        if body.is_some() {
            headers.typed_insert(ContentType::json());
        }

        let cx = Span::current().context();
        global::get_text_map_propagator(|propagator| {
            propagator.inject_context(&cx, &mut HeaderInjector(req.headers_mut().unwrap()))
        });

        let req = if let Some(body) = body {
            req.body(Body::from(serde_json::to_vec(&body)?))
        } else {
            req.body(Body::empty())
        };

        let resp = self.client.request(req?).await?;
        trace!(response = ?resp, "Load response");

        if resp.status() != StatusCode::OK {
            return Err(Error::RequestError(resp.status()));
        }

        let bytes = body::to_bytes(resp.into_body()).await?;

        Ok(bytes)
    }
}

#[cfg(test)]
mod tests {
    use headers::{authorization::Bearer, Authorization};
    use http::{Method, StatusCode};

    use crate::models;
    use crate::test_utils::get_mocked_gateway_server;

    use super::{Error, ServicesApiClient};

    // Make sure we handle any unexpected responses correctly
    #[tokio::test]
    async fn api_errors() {
        let server = get_mocked_gateway_server().await;

        let client = ServicesApiClient::new(server.uri().parse().unwrap());

        let err = client
            .request::<_, Vec<models::project::Response>, _>(
                Method::GET,
                "projects",
                None::<()>,
                None::<Authorization<Bearer>>,
            )
            .await
            .unwrap_err();

        assert!(matches!(err, Error::RequestError(StatusCode::UNAUTHORIZED)));
    }
}
