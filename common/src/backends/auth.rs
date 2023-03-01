use std::{convert::Infallible, future::Future, ops::Add, pin::Pin, sync::Arc};

use async_trait::async_trait;
use bytes::Bytes;
use chrono::{Duration, Utc};
use headers::{authorization::Bearer, Authorization, HeaderMapExt};
use http::{Request, Response, StatusCode, Uri};
use http_body::combinators::UnsyncBoxBody;
use hyper::{body, Body, Client};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header as JwtHeader, Validation};
use opentelemetry::global;
use opentelemetry_http::HeaderInjector;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tower::{Layer, Service};
use tracing::{error, trace, Span};
use tracing_opentelemetry::OpenTelemetrySpanExt;

use super::{
    cache::{CacheManagement, CacheManager},
    future::{ResponseFuture, StatusCodeFuture},
    headers::XShuttleAdminSecret,
};

pub const EXP_MINUTES: i64 = 5;
const ISS: &str = "shuttle";
const PUBLIC_KEY_CACHE_KEY: &str = "shuttle.public-key";

/// Layer to check the admin secret set by deployer is correct
#[derive(Clone)]
pub struct AdminSecretLayer {
    secret: String,
}

impl AdminSecretLayer {
    pub fn new(secret: String) -> Self {
        Self { secret }
    }
}

impl<S> Layer<S> for AdminSecretLayer {
    type Service = AdminSecret<S>;

    fn layer(&self, inner: S) -> Self::Service {
        AdminSecret {
            inner,
            secret: self.secret.clone(),
        }
    }
}

#[derive(Clone)]
pub struct AdminSecret<S> {
    inner: S,
    secret: String,
}

impl<S> Service<Request<Body>> for AdminSecret<S>
where
    S: Service<Request<Body>, Response = Response<UnsyncBoxBody<Bytes, axum::Error>>>
        + Send
        + 'static,
    S::Future: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = StatusCodeFuture<S::Future>;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        match req.headers().typed_try_get::<XShuttleAdminSecret>() {
            Ok(Some(secret)) if secret.0 == self.secret => {
                let future = self.inner.call(req);

                StatusCodeFuture::Poll(future)
            }
            Ok(_) => StatusCodeFuture::Code(StatusCode::UNAUTHORIZED),
            Err(_) => StatusCodeFuture::Code(StatusCode::BAD_REQUEST),
        }
    }
}

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

    /// Admin level scope to internals
    Admin,
}

#[derive(Deserialize, Serialize)]
/// Response used internally to pass around JWT token
pub struct ConvertResponse {
    pub token: String,
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
    token: Option<String>,
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
                &JwtHeader::new(jsonwebtoken::Algorithm::EdDSA),
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
                    jsonwebtoken::errors::ErrorKind::InvalidSignature
                    | jsonwebtoken::errors::ErrorKind::InvalidAlgorithmName
                    | jsonwebtoken::errors::ErrorKind::ExpiredSignature
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

/// Trait to get a public key asyncronously
#[async_trait]
pub trait PublicKeyFn: Send + Sync + Clone {
    type Error: std::error::Error + Send;

    async fn public_key(&self) -> Result<Vec<u8>, Self::Error>;
}

#[async_trait]
impl<F, O> PublicKeyFn for F
where
    F: Fn() -> O + Sync + Send + Clone,
    O: Future<Output = Vec<u8>> + Send,
{
    type Error = Infallible;

    async fn public_key(&self) -> Result<Vec<u8>, Self::Error> {
        Ok((self)().await)
    }
}

#[derive(Clone)]
pub struct AuthPublicKey {
    auth_uri: Uri,
    cache_manager: Arc<Box<dyn CacheManagement<Value = Vec<u8>>>>,
}

impl AuthPublicKey {
    pub fn new(auth_uri: Uri) -> Self {
        let public_key_cache_manager = CacheManager::new(1);
        Self {
            auth_uri,
            cache_manager: Arc::new(Box::new(public_key_cache_manager)),
        }
    }
}

#[async_trait]
impl PublicKeyFn for AuthPublicKey {
    type Error = PublicKeyFnError;

    async fn public_key(&self) -> Result<Vec<u8>, Self::Error> {
        if let Some(public_key) = self.cache_manager.get(PUBLIC_KEY_CACHE_KEY) {
            trace!("found public key in the cache, returning it");

            Ok(public_key)
        } else {
            let client = Client::new();
            let uri: Uri = format!("{}public-key", self.auth_uri).parse()?;
            let mut request = Request::builder().uri(uri);

            // Safe to unwrap since we just build it
            let headers = request.headers_mut().unwrap();

            let cx = Span::current().context();
            global::get_text_map_propagator(|propagator| {
                propagator.inject_context(&cx, &mut HeaderInjector(headers))
            });

            let res = client.request(request.body(Body::empty())?).await?;
            let buf = body::to_bytes(res).await?;

            trace!("inserting public key from auth service into cache");
            self.cache_manager.insert(
                PUBLIC_KEY_CACHE_KEY,
                buf.to_vec(),
                std::time::Duration::from_secs(60),
            );

            Ok(buf.to_vec())
        }
    }
}

#[derive(Debug, Error)]
pub enum PublicKeyFnError {
    #[error("invalid uri: {0}")]
    InvalidUri(#[from] http::uri::InvalidUri),

    #[error("hyper error: {0}")]
    Hyper(#[from] hyper::Error),

    #[error("http error: {0}")]
    Http(#[from] http::Error),
}

/// Layer to validate JWT tokens with a public key. Valid claims are added to the request extension
///
/// It can also be used with tonic. See:
/// https://github.com/hyperium/tonic/blob/master/examples/src/tower/server.rs
#[derive(Clone)]
pub struct JwtAuthenticationLayer<F> {
    /// User provided function to get the public key from
    public_key_fn: F,
}

impl<F: PublicKeyFn> JwtAuthenticationLayer<F> {
    /// Create a new layer to validate JWT tokens with the given public key
    pub fn new(public_key_fn: F) -> Self {
        Self { public_key_fn }
    }
}

impl<S, F: PublicKeyFn> Layer<S> for JwtAuthenticationLayer<F> {
    type Service = JwtAuthentication<S, F>;

    fn layer(&self, inner: S) -> Self::Service {
        JwtAuthentication {
            inner,
            public_key_fn: self.public_key_fn.clone(),
        }
    }
}

/// Middleware for validating a valid JWT token is present on "authorization: bearer <token>"
#[derive(Clone)]
pub struct JwtAuthentication<S, F> {
    inner: S,
    public_key_fn: F,
}

impl<S, F, ResponseError> Service<Request<Body>> for JwtAuthentication<S, F>
where
    S: Service<Request<Body>, Response = Response<UnsyncBoxBody<Bytes, ResponseError>>>
        + Send
        + Clone
        + 'static,
    S::Future: Send + 'static,
    F: PublicKeyFn + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future =
        Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send + 'static>>;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut req: Request<Body>) -> Self::Future {
        match req.headers().typed_try_get::<Authorization<Bearer>>() {
            Ok(Some(bearer)) => {
                let mut this = self.clone();

                Box::pin(async move {
                    match this.public_key_fn.public_key().await {
                        Ok(public_key) => {
                            match Claim::from_token(bearer.token().trim(), &public_key) {
                                Ok(claim) => {
                                    req.extensions_mut().insert(claim);

                                    this.inner.call(req).await
                                }
                                Err(code) => {
                                    error!(code = %code, "failed to decode JWT");

                                    Ok(Response::builder()
                                        .status(code)
                                        .body(Default::default())
                                        .unwrap())
                                }
                            }
                        }
                        Err(error) => {
                            error!(
                                error = &error as &dyn std::error::Error,
                                "failed to get public key from auth service"
                            );

                            Ok(Response::builder()
                                .status(StatusCode::SERVICE_UNAVAILABLE)
                                .body(Default::default())
                                .unwrap())
                        }
                    }
                })
            }
            Ok(None) => {
                let future = self.inner.call(req);

                Box::pin(async move { future.await })
            }
            Err(_) => Box::pin(async move {
                Ok(Response::builder()
                    .status(StatusCode::BAD_REQUEST)
                    .body(Default::default())
                    .unwrap())
            }),
        }
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

        ResponseFuture { future }
    }
}

/// Check that the required scopes are set on the [Claim] extension on a [Request]
#[derive(Clone)]
pub struct ScopedLayer {
    required: Vec<Scope>,
}

impl ScopedLayer {
    /// Scopes required to authenticate a request
    pub fn new(required: Vec<Scope>) -> Self {
        Self { required }
    }
}

impl<S> Layer<S> for ScopedLayer {
    type Service = Scoped<S>;

    fn layer(&self, inner: S) -> Self::Service {
        Scoped {
            inner,
            required: self.required.clone(),
        }
    }
}

#[derive(Clone)]
pub struct Scoped<S> {
    inner: S,
    required: Vec<Scope>,
}

impl<S> Service<Request<Body>> for Scoped<S>
where
    S: Service<Request<Body>, Response = http::Response<UnsyncBoxBody<bytes::Bytes, axum::Error>>>
        + Send
        + Clone
        + 'static,
    S::Future: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = StatusCodeFuture<S::Future>;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        let Some(claim) = req.extensions().get::<Claim>() else {
            error!("claim extension is not set");

            return StatusCodeFuture::Code(StatusCode::UNAUTHORIZED);
        };

        if self
            .required
            .iter()
            .all(|scope| claim.scopes.contains(scope))
        {
            let response_future = self.inner.call(req);
            StatusCodeFuture::Poll(response_future)
        } else {
            StatusCodeFuture::Code(StatusCode::FORBIDDEN)
        }
    }
}

#[cfg(test)]
mod tests {
    use axum::{routing::get, Extension, Router};
    use http::{Request, StatusCode};
    use hyper::{body, Body};
    use jsonwebtoken::EncodingKey;
    use ring::{
        hmac, rand,
        signature::{self, Ed25519KeyPair, KeyPair},
    };
    use serde_json::json;
    use tower::{ServiceBuilder, ServiceExt};

    use super::{Claim, JwtAuthenticationLayer, Scope, ScopedLayer};

    #[test]
    fn to_token_and_back() {
        let mut claim = Claim::new(
            "ferries".to_string(),
            vec![Scope::Deployment, Scope::Project],
        );

        let doc = signature::Ed25519KeyPair::generate_pkcs8(&rand::SystemRandom::new()).unwrap();
        let encoding_key = EncodingKey::from_ed_der(doc.as_ref());
        let token = claim.clone().into_token(&encoding_key).unwrap();

        // Make sure the token is set
        claim.token = Some(token.clone());

        let pair = Ed25519KeyPair::from_pkcs8(doc.as_ref()).unwrap();
        let public_key = pair.public_key().as_ref();

        let new = Claim::from_token(&token, public_key).unwrap();

        assert_eq!(claim, new);
    }

    #[tokio::test]
    async fn authorization_layer() {
        let claim = Claim::new(
            "ferries".to_string(),
            vec![Scope::Deployment, Scope::Project],
        );

        let doc = signature::Ed25519KeyPair::generate_pkcs8(&rand::SystemRandom::new()).unwrap();
        let encoding_key = EncodingKey::from_ed_der(doc.as_ref());
        let pair = Ed25519KeyPair::from_pkcs8(doc.as_ref()).unwrap();
        let public_key = pair.public_key().as_ref().to_vec();

        let router =
            Router::new()
                .route(
                    "/",
                    get(|Extension(claim): Extension<Claim>| async move {
                        format!("Hello, {}", claim.sub)
                    }),
                )
                .layer(
                    ServiceBuilder::new()
                        .layer(JwtAuthenticationLayer::new(move || {
                            let public_key = public_key.clone();
                            async move { public_key.clone() }
                        }))
                        .layer(ScopedLayer::new(vec![Scope::Project])),
                );

        //////////////////////////////////////////////////////////////////////////
        // Test token missing
        //////////////////////////////////////////////////////////////////////////
        let response = router
            .clone()
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        //////////////////////////////////////////////////////////////////////////
        // Test bearer missing
        //////////////////////////////////////////////////////////////////////////
        let token = claim.clone().into_token(&encoding_key).unwrap();
        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/")
                    .header("authorization", token.clone())
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        //////////////////////////////////////////////////////////////////////////
        // Test valid
        //////////////////////////////////////////////////////////////////////////
        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/")
                    .header("authorization", format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        //////////////////////////////////////////////////////////////////////////
        // Test valid extra padding
        //////////////////////////////////////////////////////////////////////////
        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/")
                    .header("Authorization", format!("Bearer   {token}   "))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = body::to_bytes(response.into_body()).await.unwrap();

        assert_eq!(&body[..], b"Hello, ferries");
    }

    // Test changing to a symmetric key is not possible
    #[test]
    #[should_panic(expected = "value: 400")]
    fn hack_symmetric_alg() {
        let claim = Claim::new(
            "hacker-hs256".to_string(),
            vec![Scope::Deployment, Scope::Project],
        );

        let doc = signature::Ed25519KeyPair::generate_pkcs8(&rand::SystemRandom::new()).unwrap();
        let encoding_key = EncodingKey::from_ed_der(doc.as_ref());
        let token = claim.into_token(&encoding_key).unwrap();

        let (header, rest) = token.split_once('.').unwrap();
        let header = base64::decode_config(header, base64::URL_SAFE_NO_PAD).unwrap();
        let mut header: serde_json::Map<String, serde_json::Value> =
            serde_json::from_slice(&header).unwrap();

        header["alg"] = json!("HS256");

        let header = serde_json::to_vec(&header).unwrap();
        let header = base64::encode_config(header, base64::URL_SAFE_NO_PAD);

        let (claim, _sig) = rest.split_once('.').unwrap();

        let msg = format!("{header}.{claim}");

        let pair = Ed25519KeyPair::from_pkcs8(doc.as_ref()).unwrap();
        let public_key = pair.public_key().as_ref();

        let sig = hmac::sign(
            &hmac::Key::new(hmac::HMAC_SHA256, pair.public_key().as_ref()),
            msg.as_bytes(),
        );
        let sig = base64::encode_config(sig, base64::URL_SAFE_NO_PAD);
        let token = format!("{msg}.{sig}");

        Claim::from_token(&token, public_key).unwrap();
    }

    // Test removing the alg is not possible
    #[test]
    #[should_panic(expected = "value: 400")]
    fn hack_no_alg() {
        let claim = Claim::new(
            "hacker-no-alg".to_string(),
            vec![Scope::Deployment, Scope::Project],
        );

        let doc = signature::Ed25519KeyPair::generate_pkcs8(&rand::SystemRandom::new()).unwrap();
        let encoding_key = EncodingKey::from_ed_der(doc.as_ref());
        let token = claim.into_token(&encoding_key).unwrap();

        let (header, rest) = token.split_once('.').unwrap();
        let header = base64::decode_config(header, base64::URL_SAFE_NO_PAD).unwrap();
        let (claim, _sig) = rest.split_once('.').unwrap();
        let mut header: serde_json::Map<String, serde_json::Value> =
            serde_json::from_slice(&header).unwrap();

        header["alg"] = json!("none");

        let header = serde_json::to_vec(&header).unwrap();
        let header = base64::encode_config(header, base64::URL_SAFE_NO_PAD);

        let token = format!("{header}.{claim}.");

        let pair = Ed25519KeyPair::from_pkcs8(doc.as_ref()).unwrap();
        let public_key = pair.public_key().as_ref();

        Claim::from_token(&token, public_key).unwrap();
    }

    // Test removing the signature is not possible
    #[test]
    #[should_panic(expected = "value: 401")]
    fn hack_no_sig() {
        let claim = Claim::new(
            "hacker-no-sig".to_string(),
            vec![Scope::Deployment, Scope::Project],
        );

        let doc = signature::Ed25519KeyPair::generate_pkcs8(&rand::SystemRandom::new()).unwrap();
        let encoding_key = EncodingKey::from_ed_der(doc.as_ref());
        let token = claim.into_token(&encoding_key).unwrap();

        let (rest, _sig) = token.rsplit_once('.').unwrap();

        let token = format!("{rest}.");

        let pair = Ed25519KeyPair::from_pkcs8(doc.as_ref()).unwrap();
        let public_key = pair.public_key().as_ref();

        Claim::from_token(&token, public_key).unwrap();
    }

    // Test changing the issuer is not possible
    #[test]
    #[should_panic(expected = "value: 401")]
    fn hack_bad_iss() {
        let claim = Claim::new(
            "hacker-iss".to_string(),
            vec![Scope::Deployment, Scope::Project],
        );

        let doc = signature::Ed25519KeyPair::generate_pkcs8(&rand::SystemRandom::new()).unwrap();
        let encoding_key = EncodingKey::from_ed_der(doc.as_ref());
        let token = claim.into_token(&encoding_key).unwrap();

        let (header, rest) = token.split_once('.').unwrap();
        let (claim, _sig) = rest.split_once('.').unwrap();
        let claim = base64::decode_config(claim, base64::URL_SAFE_NO_PAD).unwrap();
        let mut claim: serde_json::Map<String, serde_json::Value> =
            serde_json::from_slice(&claim).unwrap();

        claim["iss"] = json!("clone");

        let claim = serde_json::to_vec(&claim).unwrap();
        let claim = base64::encode_config(claim, base64::URL_SAFE_NO_PAD);

        let msg = format!("{header}.{claim}");

        let pair = Ed25519KeyPair::from_pkcs8(doc.as_ref()).unwrap();
        let public_key = pair.public_key().as_ref();

        let sig = pair.sign(msg.as_bytes());
        let sig = base64::encode_config(sig, base64::URL_SAFE_NO_PAD);
        let token = format!("{msg}.{sig}");

        Claim::from_token(&token, public_key).unwrap();
    }
}
