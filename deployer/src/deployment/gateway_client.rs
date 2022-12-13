use hyper::{body, client::HttpConnector, Body, Client, Method, Request, Uri};
use serde::{de::DeserializeOwned, Serialize};
use thiserror::Error;
use tracing::trace;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Hyper error: {0}")]
    Hyper(#[from] hyper::Error),
    #[error("Serde JSON error: {0}")]
    SerdeJson(#[from] serde_json::Error),
    #[error("Hyper error: {0}")]
    Http(#[from] hyper::http::Error),
}

/// Handles all calls to gateway
#[derive(Clone)]
pub struct GatewayClient {
    client: Client<HttpConnector>,
    base: Uri,
}

impl GatewayClient {
    pub fn new(uri: Uri) -> Self {
        Self {
            client: Client::new(),
            base: uri,
        }
    }

    /// Make a post request to a gateway endpoint
    pub async fn post<B: Serialize, T: DeserializeOwned>(
        &self,
        path: &str,
        body: Option<&B>,
    ) -> Result<T, Error> {
        self.request(Method::POST, path, body).await
    }

    /// Make a delete request to a gateway endpoint
    pub async fn delete<B: Serialize, T: DeserializeOwned>(
        &self,
        path: &str,
        body: Option<&B>,
    ) -> Result<T, Error> {
        self.request(Method::DELETE, path, body).await
    }

    async fn request<B: Serialize, T: DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        body: Option<&B>,
    ) -> Result<T, Error> {
        let uri = format!("{}{path}", self.base);
        trace!(uri, "calling gateway");

        let req = Request::builder()
            .method(method)
            .uri(uri)
            .header("Content-Type", "application/json");

        let req = if let Some(body) = body {
            req.body(Body::from(serde_json::to_vec(body)?))
        } else {
            req.body(Body::empty())
        };

        let resp = self.client.request(req?).await?;

        trace!(response = ?resp, "Load response");

        let body = resp.into_body();
        let bytes = body::to_bytes(body).await?;
        let json = serde_json::from_slice(&bytes.to_vec())?;

        Ok(json)
    }
}
