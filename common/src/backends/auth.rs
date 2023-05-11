use std::{convert::Infallible, future::Future, pin::Pin, sync::Arc, task::Poll};

use async_trait::async_trait;
use bytes::Bytes;
use headers::{authorization::Bearer, Authorization, HeaderMapExt};
use http::{Request, Response, StatusCode, Uri};
use http_body::combinators::UnsyncBoxBody;
use hyper::{body, Body, Client};
use opentelemetry::global;
use opentelemetry_http::HeaderInjector;
use pin_project::pin_project;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tower::{Layer, Service};
use tracing::{error, trace, Span};
use tracing_opentelemetry::OpenTelemetrySpanExt;

use crate::claims::{Claim, Scope};

use super::{
    cache::{CacheManagement, CacheManager},
    future::StatusCodeFuture,
    headers::XShuttleAdminSecret,
};

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

#[derive(Deserialize, Serialize)]
/// Response used internally to pass around JWT token
pub struct ConvertResponse {
    pub token: String,
}

/// Trait to get a public key asynchronously
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

type AsyncTraitFuture<A> = Pin<Box<dyn Future<Output = A> + Send>>;

#[pin_project(project = JwtAuthenticationFutureProj, project_replace = JwtAuthenticationFutureProjOwn)]
pub enum JwtAuthenticationFuture<
    PubKeyFn: PublicKeyFn,
    TService: Service<Request<Body>, Response = Response<UnsyncBoxBody<Bytes, ResponseError>>>,
    ResponseError,
> {
    // If there was an error return a BAD_REQUEST.
    Error,

    WaitForFuture {
        #[pin]
        future: TService::Future,
    },

    // We have a token and need to run our logic.
    HasTokenWaitingForPublicKey {
        bearer: Authorization<Bearer>,
        request: Request<Body>,
        #[pin]
        public_key_future: AsyncTraitFuture<Result<Vec<u8>, PubKeyFn::Error>>,
        service: TService,
    },
}

impl<PubKeyFn, TService, ResponseError> Future
    for JwtAuthenticationFuture<PubKeyFn, TService, ResponseError>
where
    PubKeyFn: PublicKeyFn + 'static,
    TService: Service<Request<Body>, Response = Response<UnsyncBoxBody<Bytes, ResponseError>>>,
{
    type Output = Result<TService::Response, TService::Error>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        match self.as_mut().project() {
            JwtAuthenticationFutureProj::Error => {
                let response = Response::builder()
                    .status(StatusCode::BAD_REQUEST)
                    .body(Default::default())
                    .unwrap();
                Poll::Ready(Ok(response))
            }
            JwtAuthenticationFutureProj::WaitForFuture { future } => future.poll(cx),
            JwtAuthenticationFutureProj::HasTokenWaitingForPublicKey {
                bearer,
                public_key_future,
                ..
            } => {
                match public_key_future.poll(cx) {
                    Poll::Pending => Poll::Pending,
                    Poll::Ready(Err(error)) => {
                        error!(
                            error = &error as &dyn std::error::Error,
                            "failed to get public key from auth service"
                        );
                        let response = Response::builder()
                            .status(StatusCode::SERVICE_UNAVAILABLE)
                            .body(Default::default())
                            .unwrap();

                        Poll::Ready(Ok(response))
                    }
                    Poll::Ready(Ok(public_key)) => {
                        let claim_result = Claim::from_token(bearer.token().trim(), &public_key);
                        match claim_result {
                            Err(code) => {
                                error!(code = %code, "failed to decode JWT");

                                let response = Response::builder()
                                    .status(code)
                                    .body(Default::default())
                                    .unwrap();

                                Poll::Ready(Ok(response))
                            }
                            Ok(claim) => {
                                let owned = self
                                    .as_mut()
                                    .project_replace(JwtAuthenticationFuture::Error);
                                match owned {
                                    JwtAuthenticationFutureProjOwn::HasTokenWaitingForPublicKey {
                                        mut request, mut service, ..
                                    } => {
                                        request.extensions_mut().insert(claim);
                                        let future = service.call(request);
                                        self.as_mut().set(JwtAuthenticationFuture::WaitForFuture { future });
                                        self.poll(cx)
                                    },
                                    _ => unreachable!("We know that we're in the 'HasTokenWaitingForPublicKey' state"),
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

impl<S, F, ResponseError> Service<Request<Body>> for JwtAuthentication<S, F>
where
    S: Service<Request<Body>, Response = Response<UnsyncBoxBody<Bytes, ResponseError>>>
        + Send
        + Clone
        + 'static,
    S::Future: Send + 'static,
    F: PublicKeyFn + 'static,
    <F as PublicKeyFn>::Error: 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = JwtAuthenticationFuture<F, S, ResponseError>;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        match req.headers().typed_try_get::<Authorization<Bearer>>() {
            Ok(Some(bearer)) => {
                let public_key_fn = self.public_key_fn.clone();
                let public_key_future = Box::pin(async move { public_key_fn.public_key().await });
                Self::Future::HasTokenWaitingForPublicKey {
                    bearer,
                    request: req,
                    public_key_future,
                    service: self.inner.clone(),
                }
            }
            Ok(None) => {
                let future = self.inner.call(req);

                Self::Future::WaitForFuture { future }
            }
            Err(_) => Self::Future::Error,
        }
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

    use crate::claims::{Claim, Scope};

    use super::{JwtAuthenticationLayer, ScopedLayer};

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
