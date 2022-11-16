use std::collections::HashMap;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;

use axum::body::boxed;
use axum::response::Response;
use fqdn::Fqdn;
use futures::future::BoxFuture;
use hyper::{Body, Request};
use instant_acme::{
    Account, AccountCredentials, Authorization, AuthorizationStatus, ChallengeType, Identifier,
    KeyAuthorization, LetsEncrypt, NewAccount, NewOrder, Order, OrderStatus,
};
use rcgen::{Certificate, CertificateParams, DistinguishedName};
use tokio::sync::Mutex;
use tokio::time::sleep;
use tower::{Layer, Service};
use tracing::{error, trace};

use crate::{Error, ProjectName};

#[derive(Debug, Eq, PartialEq)]
pub struct CustomDomain {
    pub project_name: ProjectName,
    pub certificate: Vec<u8>,
    pub private_key: Vec<u8>,
}

/// An ACME client implementation that completes Http01 challenges
/// It is safe to clone this type as it functions as a singleton
#[derive(Clone, Default)]
pub struct AcmeClient(Arc<Mutex<HashMap<String, KeyAuthorization>>>);

impl AcmeClient {
    pub fn new() -> Self {
        Self(Arc::new(Mutex::new(HashMap::default())))
    }

    async fn add_http01_challenge_authorization(&self, token: String, key: KeyAuthorization) {
        trace!(token, "saving acme http01 challenge");
        self.0.lock().await.insert(token, key);
    }

    async fn get_http01_challenge_authorization(&self, token: &str) -> Option<String> {
        self.0
            .lock()
            .await
            .get(token)
            .map(|key| key.as_str().to_owned())
    }

    async fn remove_http01_challenge_authorization(&self, token: &str) {
        trace!(token, "removing acme http01 challenge");
        self.0.lock().await.remove(token);
    }

    /// Create a new ACME account that can be restored using by deserializing the returned JSON into a [instant_acme::AccountCredentials]
    pub async fn create_account(
        &self,
        email: &str,
        acme_server: Option<String>,
    ) -> Result<serde_json::Value, AcmeClientError> {
        let acme_server = acme_server.unwrap_or_else(|| LetsEncrypt::Production.url().to_string());

        trace!(email, acme_server, "creating acme account");

        let account: NewAccount = NewAccount {
            contact: &[&format!("mailto:{email}")],
            terms_of_service_agreed: true,
            only_return_existing: false,
        };

        let account = Account::create(&account, &acme_server)
            .await
            .map_err(|error| {
                error!(%error, "got error while creating acme account");
                AcmeClientError::AccountCreation
            })?;

        let credentials = serde_json::to_value(account.credentials()).map_err(|error| {
            error!(%error, "got error while extracting credentials from acme account");
            AcmeClientError::Serializing
        })?;

        Ok(credentials)
    }

    /// Create a certificate and return it with the keys used to sign it
    pub async fn create_certificate(
        &self,
        fqdn: &Fqdn,
        credentials: AccountCredentials<'_>,
    ) -> Result<(String, Certificate), AcmeClientError> {
        let fqdn = fqdn.to_string();
        trace!(fqdn, "requesting acme certificate");

        let account = Account::from_credentials(credentials).map_err(|error| {
            error!(
                error = &error as &dyn std::error::Error,
                "failed to convert acme credentials into account"
            );
            AcmeClientError::AccountCreation
        })?;

        let (mut order, state) = account
            .new_order(&NewOrder {
                identifiers: &[Identifier::Dns(fqdn.to_string())],
            })
            .await
            .map_err(|error| {
                error!(%error, "failed to order certificate");
                AcmeClientError::OrderCreation
            })?;

        let authorizations =
            order
                .authorizations(&state.authorizations)
                .await
                .map_err(|error| {
                    error!(%error, "failed to get authorizations information");
                    AcmeClientError::AuthorizationCreation
                })?;

        // There should only ever be 1 authorization as we only provide 1 domain at a time
        debug_assert!(authorizations.len() == 1);
        let authorization = &authorizations[0];

        trace!(?authorization, "got authorization");

        self.complete_challenge(authorization, &mut order).await?;

        let Identifier::Dns(identifier) = &authorization.identifier;

        let certificate = {
            let mut params = CertificateParams::new(vec![identifier.to_string()]);
            params.distinguished_name = DistinguishedName::new();
            Certificate::from_params(params).map_err(|error| {
                error!(%error, "failed to create certificate");
                AcmeClientError::CertificateCreation
            })?
        };
        let signing_request = certificate.serialize_request_der().map_err(|error| {
            error!(%error, "failed to create certificate signing request");
            AcmeClientError::CertificateSigning
        })?;

        let certificate_chain = order
            .finalize(&signing_request, &state.finalize)
            .await
            .map_err(|error| {
                error!(%error, "failed to finalize certificate request");
                AcmeClientError::OrderFinalizing
            })?;

        Ok((certificate_chain, certificate))
    }

    async fn complete_challenge(
        &self,
        authorization: &Authorization,
        order: &mut Order,
    ) -> Result<(), AcmeClientError> {
        // Don't complete challenge for orders that are already valid
        if let AuthorizationStatus::Valid = authorization.status {
            return Ok(());
        }

        let challenge = authorization
            .challenges
            .iter()
            .find(|c| c.r#type == ChallengeType::Http01)
            .ok_or_else(|| {
                error!("http-01 challenge not found");
                AcmeClientError::MissingHttp01Challenge
            })?;

        trace!(?challenge, "will complete challenge");

        self.add_http01_challenge_authorization(
            challenge.token.clone(),
            order.key_authorization(challenge),
        )
        .await;

        order
            .set_challenge_ready(&challenge.url)
            .await
            .map_err(|error| {
                error!(%error, "failed to mark challenge as ready");
                AcmeClientError::SetReadyFailed
            })?;

        // Exponential backoff until order changes status
        let mut tries = 1u8;
        let mut delay = Duration::from_millis(250);
        let state = loop {
            sleep(delay).await;
            let state = order.state().await.map_err(|error| {
                error!(%error, "got error while fetching state");
                AcmeClientError::FetchingState
            })?;

            trace!(?state, "order state refreshed");
            match state.status {
                OrderStatus::Ready => break state,
                OrderStatus::Invalid => {
                    self.remove_http01_challenge_authorization(&challenge.token)
                        .await;
                    return Err(AcmeClientError::ChallengeInvalid);
                }
                OrderStatus::Pending => {
                    delay *= 2;
                    tries += 1;
                    if tries < 5 {
                        trace!(?state, tries, attempt_in=?delay, "order not yet ready");
                    } else {
                        error!(?state, tries, "order not ready in 5 tries");
                        self.remove_http01_challenge_authorization(&challenge.token)
                            .await;
                        return Err(AcmeClientError::ChallengeTimeout);
                    }
                }
                _ => unreachable!(),
            }
        };

        trace!(challenge.token, ?state, "challenge completed");

        self.remove_http01_challenge_authorization(&challenge.token)
            .await;

        Ok(())
    }
}

#[derive(Debug, strum::Display)]
pub enum AcmeClientError {
    AccountCreation,
    AuthorizationCreation,
    CertificateCreation,
    CertificateSigning,
    ChallengeInvalid,
    ChallengeTimeout,
    FetchingState,
    OrderCreation,
    OrderFinalizing,
    MissingHttp01Challenge,
    Serializing,
    SetReadyFailed,
}

impl std::error::Error for AcmeClientError {}

pub struct ChallengeResponderLayer {
    client: AcmeClient,
}

impl ChallengeResponderLayer {
    pub fn new(client: AcmeClient) -> Self {
        Self { client }
    }
}

impl<S> Layer<S> for ChallengeResponderLayer {
    type Service = ChallengeResponder<S>;

    fn layer(&self, inner: S) -> Self::Service {
        ChallengeResponder {
            client: self.client.clone(),
            inner,
        }
    }
}

pub struct ChallengeResponder<S> {
    client: AcmeClient,
    inner: S,
}

impl<ReqBody, S> Service<Request<ReqBody>> for ChallengeResponder<S>
where
    S: Service<Request<ReqBody>, Response = Response, Error = Error> + Send + 'static,
    S::Future: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<ReqBody>) -> Self::Future {
        if !req.uri().path().starts_with("/.well-known/acme-challenge/") {
            let future = self.inner.call(req);
            return Box::pin(async move {
                let response: Response = future.await?;
                Ok(response)
            });
        }

        let token = match req
            .uri()
            .path()
            .strip_prefix("/.well-known/acme-challenge/")
        {
            Some(token) => token.to_string(),
            None => {
                return Box::pin(async {
                    Ok(Response::builder()
                        .status(404)
                        .body(boxed(Body::empty()))
                        .unwrap())
                })
            }
        };

        trace!(token, "responding to certificate challenge");

        let client = self.client.clone();

        Box::pin(async move {
            let (status, body) = match client.get_http01_challenge_authorization(&token).await {
                Some(key) => (200, Body::from(key)),
                None => (404, Body::empty()),
            };

            Ok(Response::builder()
                .status(status)
                .body(boxed(body))
                .unwrap())
        })
    }
}
