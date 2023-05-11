use std::{
    future::Future,
    ops::Add,
    pin::Pin,
    task::{Context, Poll},
};

use bytes::Bytes;
use chrono::{Duration, Utc};
use headers::{Authorization, HeaderMapExt};
use http::{Request, StatusCode};
use http_body::combinators::UnsyncBoxBody;
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use opentelemetry::global;
use opentelemetry_http::HeaderInjector;
use pin_project::pin_project;
use serde::{Deserialize, Serialize};
use tower::{Layer, Service};
use tracing::{error, trace, Span};
use tracing_opentelemetry::OpenTelemetrySpanExt;

/// Minutes before a claim expires
///
/// We don't use the convention of 5 minutes because builds can take longer than 5 minutes. When this happens, requests
/// to provisioner will fail as the token expired.
pub const EXP_MINUTES: i64 = 15;
const ISS: &str = "shuttle";

/// The scope of operations that can be performed on shuttle
/// Every scope defaults to read and will use a suffix for updating tasks
#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Scope {
    /// Read the details, such as status and address, of a deployment
    Deployment,

    /// Push a new deployment
    DeploymentPush,

    /// Read the logs of a deployment
    Logs,

    /// Read the details of a service
    Service,

    /// Create a new service
    ServiceCreate,

    /// Read the status of a project
    Project,

    /// Create a new project
    ProjectCreate,

    /// Get the resources for a project
    Resources,

    /// Provision new resources for a project or update existing ones
    ResourcesWrite,

    /// List the secrets of a project
    Secret,

    /// Add or update secrets of a project
    SecretWrite,

    /// Get list of users
    User,

    /// Add or update users
    UserCreate,

    /// Create an ACME account
    AcmeCreate,

    /// Create a custom domain,
    CustomDomainCreate,

    /// Renew the certificate of a custom domain.
    CustomDomainCertificateRenew,

    /// Request renewal of the gateway certificate.
    /// Note: this step should be completed manually in terms
    /// of DNS-01 challenge completion.
    GatewayCertificateRenew,

    /// Admin level scope to internals
    Admin,
}

pub struct ScopeBuilder(Vec<Scope>);

impl ScopeBuilder {
    /// Create a builder with the standard scopes for new users.
    pub fn new() -> Self {
        Self(vec![
            Scope::Deployment,
            Scope::DeploymentPush,
            Scope::Logs,
            Scope::Service,
            Scope::ServiceCreate,
            Scope::Project,
            Scope::ProjectCreate,
            Scope::Resources,
            Scope::ResourcesWrite,
            Scope::Secret,
            Scope::SecretWrite,
        ])
    }

    /// Extend the current scopes with admin scopes.
    pub fn with_admin(mut self) -> Self {
        self.0.extend(vec![
            Scope::User,
            Scope::UserCreate,
            Scope::AcmeCreate,
            Scope::CustomDomainCreate,
            Scope::CustomDomainCertificateRenew,
            Scope::GatewayCertificateRenew,
            Scope::Admin,
        ]);
        self
    }

    pub fn build(self) -> Vec<Scope> {
        self.0
    }
}

impl Default for ScopeBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
pub struct Claim {
    /// Expiration time (as UTC timestamp).
    pub exp: usize,
    /// Issued at (as UTC timestamp).
    iat: usize,
    /// Issuer.
    iss: String,
    /// Not Before (as UTC timestamp).
    nbf: usize,
    /// Subject (whom token refers to).
    pub sub: String,
    /// Scopes this token can access
    pub scopes: Vec<Scope>,
    /// The original token that was parsed
    pub(crate) token: Option<String>,
}

impl Claim {
    /// Create a new claim for a user with the given scopes
    pub fn new(sub: String, scopes: Vec<Scope>) -> Self {
        let iat = Utc::now();
        let exp = iat.add(Duration::minutes(EXP_MINUTES));

        Self {
            exp: exp.timestamp() as usize,
            iat: iat.timestamp() as usize,
            iss: ISS.to_string(),
            nbf: iat.timestamp() as usize,
            sub,
            scopes,
            token: None,
        }
    }

    pub fn into_token(self, encoding_key: &EncodingKey) -> Result<String, StatusCode> {
        if let Some(token) = self.token {
            Ok(token)
        } else {
            encode(
                &Header::new(jsonwebtoken::Algorithm::EdDSA),
                &self,
                encoding_key,
            )
            .map_err(|err| {
                error!(
                    error = &err as &dyn std::error::Error,
                    "failed to convert claim to token"
                );
                match err.kind() {
                    jsonwebtoken::errors::ErrorKind::Json(_) => StatusCode::INTERNAL_SERVER_ERROR,
                    jsonwebtoken::errors::ErrorKind::Crypto(_) => StatusCode::SERVICE_UNAVAILABLE,
                    _ => StatusCode::INTERNAL_SERVER_ERROR,
                }
            })
        }
    }

    pub fn from_token(token: &str, public_key: &[u8]) -> Result<Self, StatusCode> {
        let decoding_key = DecodingKey::from_ed_der(public_key);
        let mut validation = Validation::new(jsonwebtoken::Algorithm::EdDSA);
        validation.set_issuer(&[ISS]);

        trace!(token, "converting token to claim");
        let mut claim: Self = decode(token, &decoding_key, &validation)
            .map_err(|err| {
                error!(
                    error = &err as &dyn std::error::Error,
                    "failed to convert token to claim"
                );
                match err.kind() {
                    jsonwebtoken::errors::ErrorKind::ExpiredSignature => {
                        StatusCode::from_u16(499).unwrap() // Expired status code which is safe to unwrap
                    }
                    jsonwebtoken::errors::ErrorKind::InvalidSignature
                    | jsonwebtoken::errors::ErrorKind::InvalidAlgorithmName
                    | jsonwebtoken::errors::ErrorKind::InvalidIssuer
                    | jsonwebtoken::errors::ErrorKind::ImmatureSignature => {
                        StatusCode::UNAUTHORIZED
                    }
                    jsonwebtoken::errors::ErrorKind::InvalidToken
                    | jsonwebtoken::errors::ErrorKind::InvalidAlgorithm
                    | jsonwebtoken::errors::ErrorKind::Base64(_)
                    | jsonwebtoken::errors::ErrorKind::Json(_)
                    | jsonwebtoken::errors::ErrorKind::Utf8(_) => StatusCode::BAD_REQUEST,
                    jsonwebtoken::errors::ErrorKind::MissingAlgorithm => {
                        StatusCode::INTERNAL_SERVER_ERROR
                    }
                    jsonwebtoken::errors::ErrorKind::Crypto(_) => StatusCode::SERVICE_UNAVAILABLE,
                    _ => StatusCode::INTERNAL_SERVER_ERROR,
                }
            })?
            .claims;

        claim.token = Some(token.to_string());

        Ok(claim)
    }
}

// Future for layers that just return the inner response
#[pin_project]
pub struct ResponseFuture<F>(#[pin] pub F);

impl<F, Response, Error> Future for ResponseFuture<F>
where
    F: Future<Output = Result<Response, Error>>,
{
    type Output = Result<Response, Error>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();

        this.0.poll(cx)
    }
}

/// This layer takes a claim on a request extension and uses it's internal token to set the Authorization Bearer
#[derive(Clone)]
pub struct ClaimLayer;

impl<S> Layer<S> for ClaimLayer {
    type Service = ClaimService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        ClaimService { inner }
    }
}

#[derive(Clone)]
pub struct ClaimService<S> {
    inner: S,
}

impl<S, RequestError> Service<Request<UnsyncBoxBody<Bytes, RequestError>>> for ClaimService<S>
where
    S: Service<Request<UnsyncBoxBody<Bytes, RequestError>>> + Send + 'static,
    S::Future: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = ResponseFuture<S::Future>;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut req: Request<UnsyncBoxBody<Bytes, RequestError>>) -> Self::Future {
        if let Some(claim) = req.extensions().get::<Claim>() {
            if let Some(token) = claim.token.clone() {
                req.headers_mut()
                    .typed_insert(Authorization::bearer(&token).expect("to set JWT token"));
            }
        }

        let future = self.inner.call(req);

        ResponseFuture(future)
    }
}

/// This layer adds the current tracing span to any outgoing request
#[derive(Clone)]
pub struct InjectPropagationLayer;

impl<S> Layer<S> for InjectPropagationLayer {
    type Service = InjectPropagation<S>;

    fn layer(&self, inner: S) -> Self::Service {
        InjectPropagation { inner }
    }
}

#[derive(Clone)]
pub struct InjectPropagation<S> {
    inner: S,
}

impl<S, Body> Service<Request<Body>> for InjectPropagation<S>
where
    S: Service<Request<Body>> + Send + 'static,
    S::Future: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = ResponseFuture<S::Future>;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut req: Request<Body>) -> Self::Future {
        let cx = Span::current().context();

        global::get_text_map_propagator(|propagator| {
            propagator.inject_context(&cx, &mut HeaderInjector(req.headers_mut()))
        });

        let future = self.inner.call(req);

        ResponseFuture(future)
    }
}
