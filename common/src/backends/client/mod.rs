use headers::{ContentType, Header, HeaderMapExt};
use http::{Method, Request, StatusCode, Uri};
use hyper::{body, client::HttpConnector, Body, Client};
use opentelemetry::global;
use opentelemetry_http::HeaderInjector;
use serde::{de::DeserializeOwned, Serialize};
use thiserror::Error;
use tracing::{trace, Span};
use tracing_opentelemetry::OpenTelemetrySpanExt;

mod gateway;

pub use gateway::GatewayClient;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Hyper error: {0}")]
    Hyper(#[from] hyper::Error),
    #[error("Serde JSON error: {0}")]
    SerdeJson(#[from] serde_json::Error),
    #[error("Hyper error: {0}")]
    Http(#[from] hyper::http::Error),
}

/// `Hyper` wrapper to make request to RESTful Shuttle services easy
#[derive(Clone)]
pub struct ServicesApiClient {
    client: Client<HttpConnector>,
    base: Uri,
}

impl ServicesApiClient {
    /// Make a new client that to connect to the given endpoint
    fn new(base: Uri) -> Self {
        Self {
            client: Client::new(),
            base,
        }
    }

    /// Make a get request to a path on the service
    pub async fn request<B: Serialize, T: DeserializeOwned, H: Header>(
        &self,
        method: Method,
        path: &str,
        body: Option<B>,
        extra_header: Option<H>,
    ) -> Result<T, Error> {
        let uri = format!("{}{path}", self.base);
        trace!(uri, "calling inner service");

        let mut req = Request::builder().method(method).uri(uri);
        let headers = req
            .headers_mut()
            .expect("new request to have mutable headers");

        headers.typed_insert(ContentType::json());

        if let Some(extra_header) = extra_header {
            headers.typed_insert(extra_header);
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
            todo!();
        }

        let body = resp.into_body();
        let bytes = body::to_bytes(body).await?;
        let json = serde_json::from_slice(&bytes)?;

        Ok(json)
    }
}
