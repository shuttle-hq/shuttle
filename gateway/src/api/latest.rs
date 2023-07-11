use std::io::Cursor;
use std::net::SocketAddr;
use std::ops::Sub;
use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Extension, Path, Query, State};
use axum::handler::Handler;
use axum::middleware::from_extractor;
use axum::response::Response;
use axum::routing::{get, post, put};
use axum::{Json as AxumJson, Router};
use axum_extra::extract::cookie::Cookie;
use axum_extra::extract::CookieJar;
use fqdn::FQDN;
use futures::Future;
use http::header::COOKIE;
use http::{StatusCode, Uri};
use instant_acme::{AccountCredentials, ChallengeType};
use serde::{Deserialize, Serialize};
use shuttle_common::backends::auth::{JwtAuthenticationLayer, ScopedLayer, COOKIE_NAME};
use shuttle_common::backends::cache::{CacheManagement, CacheManager};
use shuttle_common::backends::metrics::{Metrics, TraceLayer};
use shuttle_common::claims::{AccountTier, InjectPropagation, Scope};
use shuttle_common::models::error::ErrorKind;
use shuttle_common::models::project;
use shuttle_common::request_span;
use shuttle_proto::auth::auth_client::AuthClient;
use shuttle_proto::auth::{
    AuthPublicKey, LogoutRequest, NewUser, ResetKeyRequest, ResultResponse, UserRequest,
};
use tonic::metadata::MetadataValue;
use tonic::transport::Channel;
use tonic::Request as TonicRequest;
use tracing::{debug, error, field, instrument};
use utoipa::IntoParams;

use utoipa::openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme};
use utoipa::{Modify, OpenApi};
use utoipa_swagger_ui::SwaggerUi;
use x509_parser::nom::AsBytes;
use x509_parser::parse_x509_certificate;
use x509_parser::pem::parse_x509_pem;
use x509_parser::time::ASN1Time;

use crate::acme::{AcmeClient, CustomDomain};
use crate::auth::{extract_metadata_cookie, insert_metadata_bearer_token, Key, ScopedUser, User};
use crate::dal::Dal;
use crate::service::GatewayService;
use crate::tls::{GatewayCertResolver, RENEWAL_VALIDITY_THRESHOLD_IN_DAYS};
use crate::{AccountName, Error, ProjectName};

use super::auth_layer::ShuttleAuthLayer;

pub const SVC_DEGRADED_THRESHOLD: usize = 128;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GatewayStatus {
    Healthy,
    Degraded,
    Unhealthy,
}

#[derive(Serialize, Deserialize)]
pub struct StatusResponse {
    status: GatewayStatus,
}

#[derive(Debug, Clone, Copy, Deserialize, IntoParams)]
pub struct PaginationDetails {
    /// Page to fetch, starting from 0.
    pub page: Option<u32>,
    /// Number of results per page.
    pub limit: Option<u32>,
}

impl StatusResponse {
    pub fn healthy() -> Self {
        Self {
            status: GatewayStatus::Healthy,
        }
    }

    pub fn degraded() -> Self {
        Self {
            status: GatewayStatus::Degraded,
        }
    }

    pub fn unhealthy() -> Self {
        Self {
            status: GatewayStatus::Unhealthy,
        }
    }
}

#[instrument]
#[utoipa::path(
    get,
    path = "/",
    responses(
        (status = 200, description = "Get the gateway operational status."),
        (status = 500, description = "Server internal error.")
    )
)]
async fn get_status() -> Response<Body> {
    let body = serde_json::to_vec(&StatusResponse::healthy()).unwrap();

    Response::builder()
        .status(StatusCode::OK)
        .body(body.into())
        .unwrap()
}

#[instrument(skip(service))]
#[utoipa::path(
    get,
    path = "/projects/{project_name}",
    responses(
        (status = 200, description = "Successfully got a specific project information.", body = shuttle_common::models::project::Response),
        (status = 500, description = "Server internal error.")
    ),
    params(
        ("project_name" = String, Path, description = "The name of the project."),
    )
)]
async fn get_project<D: Dal>(
    State(RouterState { service, .. }): State<RouterState<D>>,
    ScopedUser { scope, .. }: ScopedUser<D>,
) -> Result<AxumJson<project::Response>, Error> {
    let project_name = service.find_project(&scope).await?;

    let response = project::Response {
        name: project_name.to_string(),
        // TODO: This is hardcoded until we refactor the way we get the state of a project,
        // if we do at all in the gateway.
        state: project::State::Ready,
    };

    Ok(AxumJson(response))
}

#[instrument(skip_all, fields(%name))]
#[utoipa::path(
    get,
    path = "/projects",
    responses(
        (status = 200, description = "Successfully got the projects list.", body = [shuttle_common::models::project::Response]),
        (status = 500, description = "Server internal error.")
    ),
    params(
        PaginationDetails
    )
)]
async fn get_projects_list<D: Dal>(
    State(RouterState { service, .. }): State<RouterState<D>>,
    User { name, .. }: User<D>,
    Query(PaginationDetails { page, limit }): Query<PaginationDetails>,
) -> Result<AxumJson<Vec<project::Response>>, Error> {
    let limit = limit.unwrap_or(u32::MAX);
    let page = page.unwrap_or(0);
    let projects = service
        // The `offset` is page size * amount of pages
        .iter_user_projects_paginated(&name, limit * page, limit)
        .await?
        .map(|project_name| project::Response {
            name: project_name.to_string(),
            // TODO: This is hardcoded until we refactor the way we get the state of a project,
            // if we do at all in the gateway.
            state: project::State::Ready,
        })
        .collect();

    Ok(AxumJson(projects))
}

/// Get all projects, this requires an admin bearer token.
#[instrument(skip_all)]
#[utoipa::path(
    post,
    path = "/admin/projects",
    responses(
        (status = 200, description = "Successfully fetched the projects list.", body = shuttle_common::models::project::AdminResponse),
        (status = 500, description = "Server internal error.")
    ),
    security(
        ("api_key" = [])
    )
)]
async fn get_projects<D: Dal>(
    State(RouterState { service, .. }): State<RouterState<D>>,
) -> Result<AxumJson<Vec<project::AdminResponse>>, Error> {
    let projects = service.iter_projects().await?.map(Into::into).collect();

    Ok(AxumJson(projects))
}

// TODO: route deployer control plane requests

#[instrument(skip_all, fields(%email, ?acme_server))]
#[utoipa::path(
    post,
    path = "/admin/acme/{email}",
    responses(
        (status = 200, description = "Created an acme account.", content_type = "application/json", body = String),
        (status = 500, description = "Server internal error.")
    ),
    params(
        ("email" = String, Path, description = "An email the acme account binds to."),
    ),

)]
async fn create_acme_account(
    Extension(acme_client): Extension<AcmeClient>,
    Path(email): Path<String>,
    AxumJson(acme_server): AxumJson<Option<String>>,
) -> Result<AxumJson<serde_json::Value>, Error> {
    let res = acme_client.create_account(&email, acme_server).await?;

    Ok(AxumJson(res))
}

#[instrument(skip_all, fields(%project_name, %fqdn))]
#[utoipa::path(
    post,
    path = "/admin/acme/request/{project_name}/{fqdn}",
    responses(
        (status = 200, description = "Successfully requested a custom domain for the the project."),
        (status = 500, description = "Server internal error.")
    ),
    params(
        ("project_name" = String, Path, description = "The project name associated to the requested custom domain."),
        ("fqdn" = String, Path, description = "The fqdn that represents the requested custom domain."),
    )
)]
async fn request_custom_domain_acme_certificate<D: Dal>(
    State(RouterState { service, .. }): State<RouterState<D>>,
    Extension(acme_client): Extension<AcmeClient>,
    Extension(resolver): Extension<Arc<GatewayCertResolver>>,
    Path((project_name, fqdn)): Path<(ProjectName, String)>,
    AxumJson(credentials): AxumJson<AccountCredentials<'_>>,
) -> Result<String, Error> {
    let fqdn: FQDN = fqdn
        .parse()
        .map_err(|_err| Error::from(ErrorKind::InvalidCustomDomain))?;

    let (certs, private_key) = service
        .create_custom_domain_certificate(&fqdn, &acme_client, &project_name, credentials)
        .await?;

    let mut buf = Vec::new();
    buf.extend(certs.as_bytes());
    buf.extend(private_key.as_bytes());
    resolver
        .serve_pem(&fqdn.to_string(), Cursor::new(buf))
        .await?;
    Ok(format!(
        r#""New certificate created for {} project.""#,
        project_name
    ))
}

#[instrument(skip_all, fields(%project_name, %fqdn))]
#[utoipa::path(
    post,
    path = "/admin/acme/renew/{project_name}/{fqdn}",
    responses(
        (status = 200, description = "Successfully renewed the project TLS certificate for the appointed custom domain fqdn."),
        (status = 500, description = "Server internal error.")
    ),
    params(
        ("project_name" = String, Path, description = "The project name associated to the requested custom domain."),
        ("fqdn" = String, Path, description = "The fqdn that represents the requested custom domain."),
    )
)]
async fn renew_custom_domain_acme_certificate<D: Dal>(
    State(RouterState { service, .. }): State<RouterState<D>>,
    Extension(acme_client): Extension<AcmeClient>,
    Extension(resolver): Extension<Arc<GatewayCertResolver>>,
    Path((project_name, fqdn)): Path<(ProjectName, String)>,
    AxumJson(credentials): AxumJson<AccountCredentials<'_>>,
) -> Result<String, Error> {
    let fqdn: FQDN = fqdn
        .parse()
        .map_err(|_err| Error::from(ErrorKind::InvalidCustomDomain))?;
    // Try retrieve the current certificate if any.
    match service.project_details_for_custom_domain(&fqdn).await {
        Ok(CustomDomain { certificate, .. }) => {
            let (_, pem) = parse_x509_pem(certificate.as_bytes()).unwrap_or_else(|_| {
                panic!(
                    "Malformed existing PEM certificate for {} project.",
                    project_name
                )
            });
            let (_, x509_cert_chain) = parse_x509_certificate(pem.contents.as_bytes())
                .unwrap_or_else(|_| {
                    panic!(
                        "Malformed existing X509 certificate for {} project.",
                        project_name
                    )
                });
            let diff = x509_cert_chain
                .validity()
                .not_after
                .sub(ASN1Time::now())
                .unwrap();

            // If current certificate validity less_or_eq than 30 days, attempt renewal.
            if diff.whole_days() <= RENEWAL_VALIDITY_THRESHOLD_IN_DAYS {
                return match acme_client
                    .create_certificate(&fqdn.to_string(), ChallengeType::Http01, credentials)
                    .await
                {
                    // If successfuly created, save the certificate in memory to be
                    // served in the future.
                    Ok((certs, private_key)) => {
                        let mut buf = Vec::new();
                        buf.extend(certs.as_bytes());
                        buf.extend(private_key.as_bytes());
                        resolver
                            .serve_pem(&fqdn.to_string(), Cursor::new(buf))
                            .await?;
                        Ok(format!(
                            r#""Certificate renewed for {} project.""#,
                            project_name
                        ))
                    }
                    Err(err) => Err(err.into()),
                };
            } else {
                Ok(format!(
                    r#""Certificate renewal skipped, {} project certificate still valid for {} days.""#,
                    project_name, diff
                ))
            }
        }
        Err(err) => Err(err),
    }
}

#[instrument(skip_all)]
#[utoipa::path(
    post,
    path = "/admin/acme/gateway/renew",
    responses(
        (status = 200, description = "Successfully renewed the gateway TLS certificate."),
        (status = 500, description = "Server internal error.")
    )
)]
async fn renew_gateway_acme_certificate<D: Dal>(
    State(RouterState { service, .. }): State<RouterState<D>>,
    Extension(acme_client): Extension<AcmeClient>,
    Extension(resolver): Extension<Arc<GatewayCertResolver>>,
    AxumJson(credentials): AxumJson<AccountCredentials<'_>>,
) -> Result<String, Error> {
    service
        .renew_certificate(&acme_client, resolver, credentials)
        .await;
    Ok(r#""Renewed the gateway certificate.""#.to_string())
}

/// Login a user by their account name and return a shuttle.sid cookie, this endpoint expects the
/// api-key of an admin user as a Bearer token.
#[instrument(skip_all, fields(%account_name))]
#[utoipa::path(
    post,
    path = "/login/{account_name}",
    responses(
        (status = 200, description = "Successfully logged in user and returned cookie."),
        (status = 401, description = "Unauthorized to due to missing or invalid admin api-key."),
        (status = 500, description = "Server internal error."),
        (status = 503, description = "Server not reachable.")
    ),
    params(
        ("account_name" = AccountName, Path, description = "The account name of the user to log in."),
    ),
    security(
        ("api_key" = [])
    )
)]
async fn login<D: Dal>(
    jar: CookieJar,
    State(RouterState {
        mut auth_client, ..
    }): State<RouterState<D>>,
    key: Key,
    Path(account_name): Path<AccountName>,
) -> Result<(CookieJar, AxumJson<shuttle_common::models::user::Response>), Error> {
    let mut request = TonicRequest::new(UserRequest {
        account_name: account_name.to_string(),
    });

    // This endpoint expects the api-key of an admin user in it's bearer token.
    insert_metadata_bearer_token(request.metadata_mut(), key)?;

    let response = auth_client.login(request).await.map_err(|error| {
        debug!(error = ?error, "failed to login user");
        Error::from_kind(ErrorKind::Unauthorized)
    })?;

    let cookie = extract_metadata_cookie(response.metadata(), "login")?;

    let cookie = Cookie::parse(cookie.to_string()).map_err(|error| {
        debug!(error = ?error, "failed to parse set-cookie cookie from login request");
        Error::from_kind(ErrorKind::Internal)
    })?;

    let jar = jar.add(cookie);

    let response = response.into_inner();

    Ok((jar, AxumJson(response.into())))
}

/// Logout a user and return a shuttle.sid removal cookie. This endpoint expects a shuttle.sid
/// cookie.
#[instrument(skip_all)]
#[utoipa::path(
    post,
    path = "/logout",
    responses(
        (status = 200, description = "Successfully logged out user and returned logout cookie."),
        (status = 401, description = "Unauthorized to due to missing or invalid shuttle.sid cookie."),
        (status = 500, description = "Server internal error."),
        (status = 503, description = "Server not reachable.")
    )
)]
async fn logout<D: Dal>(
    jar: CookieJar,
    State(RouterState {
        auth_cache,
        mut auth_client,
        ..
    }): State<RouterState<D>>,
) -> Result<CookieJar, Error> {
    let cookie = jar
        .get(COOKIE_NAME)
        .ok_or(Error::from_kind(ErrorKind::CookieMissing))?;

    // This is the value in `shuttle.sid=<value>`.
    let cache_key = cookie.value();

    let mut request = TonicRequest::new(LogoutRequest::default());

    request.metadata_mut().insert(
        COOKIE.as_str(),
        MetadataValue::try_from(&cookie.to_string()).map_err(|error| {
            error!(error = ?error, "received malformed {COOKIE_NAME} cookie");

            Error::from_kind(ErrorKind::CookieMalformed)
        })?,
    );

    let response = auth_client
        .logout(request)
        .await
        .map_err(|_| Error::from_kind(ErrorKind::Internal))?;

    let logout_cookie = extract_metadata_cookie(response.metadata(), "logout")?;

    let logout_cookie = Cookie::parse(logout_cookie.to_string()).map_err(|error| {
        debug!(error = ?error, "failed to parse set-cookie cookie from logout request");
        Error::from_kind(ErrorKind::Internal)
    })?;

    // TODO: verify this is the correct key
    if auth_cache.invalidate(cache_key).is_none() {
        debug!("did not find cookie key to invalidate in auth cache for logout request");
    }

    Ok(jar.add(logout_cookie))
}

/// Fetch a user from the auth service state, this endpoint expects the api-key of an admin user as
/// a Bearer token.
#[instrument(skip_all, fields(%account_name))]
#[utoipa::path(
    get,
    path = "/users/{account_name}",
    responses(
        (status = 200, description = "Successfully retrieved user."),
        (status = 401, description = "Unauthorized to due to missing or invalid admin api-key."),
        (status = 404, description = "User not found."),
        (status = 500, description = "Server internal error."),
        (status = 503, description = "Server not reachable.")
    ),
    params(
        ("account_name" = AccountName, Path, description = "The account name of the user to get."),
    ),
    security(
        ("api_key" = [])
    )
)]
async fn get_user<D: Dal>(
    State(RouterState {
        mut auth_client, ..
    }): State<RouterState<D>>,
    Path(account_name): Path<AccountName>,
    key: Key,
) -> Result<AxumJson<shuttle_common::models::user::Response>, Error> {
    let mut request = TonicRequest::new(UserRequest {
        account_name: account_name.to_string(),
    });

    // This endpoint expects the api-key of an admin user in it's bearer token.
    insert_metadata_bearer_token(request.metadata_mut(), key)?;

    let response = auth_client
        .get_user_request(request)
        .await
        .map_err(|error| match error.code() {
            // This is an admin guarded route, if it progresses to querying a user even if it doesn't succeed
            // it is authorized. For any other failure return 401 Unauthorized.
            // TODO: should we also make the get_user_request able to return a 500 on DB error?
            tonic::Code::NotFound => Error::from_kind(ErrorKind::UserNotFound),
            _ => Error::from_kind(ErrorKind::Unauthorized),
        })?
        .into_inner();

    Ok(AxumJson(response.into()))
}

/// Insert a new user in the auth service state, this endpoint expects the api-key of an admin user as
/// a Bearer token.
#[instrument(skip_all, fields(%account_name, %account_tier))]
#[utoipa::path(
    post,
    path = "/users/{account_name}/{account_tier}",
    responses(
        (status = 200, description = "Successfully logged in user and returned cookie."),
        (status = 401, description = "Unauthorized to due to missing or invalid admin api-key."),
        (status = 404, description = "User not found."),
        (status = 500, description = "Server internal error."),
        (status = 503, description = "Server not reachable.")
    ),
    params(
        ("account_name" = AccountName, Path, description = "The account name of the new user."),
        ("account_tier" = AccountTier, Path, description = "The account tier of the new user."),
    ),
    security(
        ("api_key" = [])
    )
)]
async fn post_user<D: Dal>(
    State(RouterState {
        mut auth_client, ..
    }): State<RouterState<D>>,
    Path((account_name, account_tier)): Path<(AccountName, AccountTier)>,
    key: Key,
) -> Result<AxumJson<shuttle_common::models::user::Response>, Error> {
    let mut request = TonicRequest::new(NewUser {
        account_name: account_name.to_string(),
        account_tier: account_tier.to_string(),
    });

    // This endpoint expects the api-key of an admin user in it's bearer token.
    insert_metadata_bearer_token(request.metadata_mut(), key)?;

    let response = auth_client
        .post_user_request(request)
        .await
        .map_err(|error| match error.code() {
            tonic::Code::Internal => {
                debug!(error = ?error, "failed to create new user");

                Error::from_kind(ErrorKind::Internal)
            }
            _ => Error::from_kind(ErrorKind::Unauthorized),
        })?
        .into_inner();

    Ok(AxumJson(response.into()))
}

/// Reset the api-key of a user, this endpoint expects a shuttle.sid cookie or the api-key of the
/// user as a Bearer token.
#[instrument(skip_all)]
#[utoipa::path(
    put,
    path = "/users/reset-api-key",
    responses(
        (status = 200, description = "Successfully reset the users api-key."),
        (status = 401, description = "Unauthorized to due to missing or invalid api-key or shuttle.sid cookie."),
        (status = 500, description = "Server internal error."),
        (status = 503, description = "Server not reachable.")
    ),
    security(
        ("api_key" = [])
    )
)]
async fn reset_api_key<D: Dal>(
    State(RouterState {
        mut auth_client,
        auth_cache,
        ..
    }): State<RouterState<D>>,
    key: Option<Key>,
    jar: CookieJar,
) -> Result<(), Error> {
    let request_data = if let Some(cookie) = jar.get(COOKIE_NAME) {
        // Received request with cookie, insert it into our reset-key request and
        // use it to authorize the call.
        let mut request = TonicRequest::new(ResetKeyRequest::default());

        // This is the value in `shuttle.sid=<value>`.
        let cache_key = cookie.value();

        request.metadata_mut().insert(
            COOKIE.as_str(),
            MetadataValue::try_from(&cookie.to_string()).map_err(|error| {
                error!(error = ?error, "received malformed {COOKIE_NAME} cookie");

                Error::from_kind(ErrorKind::CookieMalformed)
            })?,
        );

        Some((request, cache_key.to_string()))
    } else if let Some(key) = key {
        // Received request with api-key bearer token, insert it into our reset-key request and
        // use it to authorize the call.
        let key = shuttle_common::ApiKey::from(key).as_ref().to_string();
        let cache_key = key.clone();

        let request = TonicRequest::new(ResetKeyRequest { api_key: Some(key) });

        Some((request, cache_key))
    } else {
        None
    };

    let Some((request, cache_key)) = request_data else {
        return Err(Error::from_kind(ErrorKind::Unauthorized));
    };

    let ResultResponse { success, message } = auth_client
        .reset_api_key(request)
        .await
        .map_err(|_| Error::from(ErrorKind::Internal))?
        .into_inner();

    if !success {
        error!(message = ?message, "failed to reset api key");
        Err(Error::from(ErrorKind::Internal))
    } else {
        if auth_cache.invalidate(&cache_key).is_none() {
            debug!("did not find cookie key to invalidate in auth cache for reset-key request");
        }
        Ok(())
    }
}

struct SecurityAddon;

impl Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                "api_key",
                SecurityScheme::Http(
                    HttpBuilder::new()
                        .scheme(HttpAuthScheme::Bearer)
                        .description(Some(
                            "Api-key bearer token used to authorize requests that call the auth service.",
                        ))
                        .build(),
                ),
            )
        }
    }
}

#[derive(OpenApi)]
#[openapi(
    paths(
        create_acme_account,
        request_custom_domain_acme_certificate,
        renew_custom_domain_acme_certificate,
        renew_gateway_acme_certificate,
        login,
        logout,
        get_user,
        post_user,
        reset_api_key
    ),
    modifiers(&SecurityAddon),
    components(schemas(
        shuttle_common::models::project::State,
        crate::AccountName,
        shuttle_common::claims::AccountTier,
    ))
)]
pub struct ApiDoc;

#[derive(Clone)]
pub(crate) struct RouterState<D: Dal> {
    pub auth_client: AuthClient<InjectPropagation<Channel>>,
    pub auth_cache: Arc<Box<dyn CacheManagement<Value = String>>>,
    pub service: Arc<GatewayService<D>>,
}

pub struct ApiBuilder<D: Dal> {
    auth_client: Option<AuthClient<InjectPropagation<Channel>>>,
    auth_cache: Option<Arc<Box<dyn CacheManagement<Value = String>>>>,
    router: Router<RouterState<D>>,
    service: Option<Arc<GatewayService<D>>>,
    bind: Option<SocketAddr>,
}

impl<D> Default for ApiBuilder<D>
where
    D: Dal + 'static,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<D> ApiBuilder<D>
where
    D: Dal + 'static,
{
    pub fn new() -> Self {
        Self {
            auth_client: None,
            auth_cache: None,
            router: Router::new(),
            service: None,
            bind: None,
        }
    }

    pub fn with_acme(mut self, acme: AcmeClient, resolver: Arc<GatewayCertResolver>) -> Self {
        self.router = self
            .router
            .route(
                "/admin/acme/:email",
                post(create_acme_account.layer(ScopedLayer::new(vec![Scope::AcmeCreate]))),
            )
            .route(
                "/admin/acme/request/:project_name/:fqdn",
                post(
                    request_custom_domain_acme_certificate
                        .layer(ScopedLayer::new(vec![Scope::CustomDomainCreate])),
                ),
            )
            .route(
                "/admin/acme/renew/:project_name/:fqdn",
                post(
                    renew_custom_domain_acme_certificate
                        .layer(ScopedLayer::new(vec![Scope::CustomDomainCertificateRenew])),
                ),
            )
            .route(
                "/admin/acme/gateway/renew",
                post(
                    renew_gateway_acme_certificate
                        .layer(ScopedLayer::new(vec![Scope::GatewayCertificateRenew])),
                ),
            )
            .layer(Extension(acme))
            .layer(Extension(resolver));
        self
    }

    pub fn with_service(mut self, service: Arc<GatewayService<D>>) -> Self {
        self.service = Some(service);
        self
    }

    pub fn binding_to(mut self, addr: SocketAddr) -> Self {
        self.bind = Some(addr);
        self
    }

    pub fn with_default_traces(mut self) -> Self {
        self.router = self.router.route_layer(from_extractor::<Metrics>()).layer(
            TraceLayer::new(|request| {
                request_span!(
                    request,
                    account.name = field::Empty,
                    request.params.project_name = field::Empty,
                    request.params.account_name = field::Empty
                )
            })
            .with_propagation()
            .build(),
        );
        self
    }

    pub fn with_default_routes(mut self) -> Self {
        let admin_routes = Router::new()
            .route("/projects", get(get_projects))
            // TODO: The `/swagger-ui` responds with a 303 See Other response which is followed in
            // browsers but leads to 404 Not Found. This must be investigated.
            .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
            .layer(ScopedLayer::new(vec![Scope::Admin]));

        self.router = self
            .router
            .route("/", get(get_status))
            .route("/logout", post(logout))
            .route("/projects/:project_name", get(get_project))
            .route("/projects", get(get_projects_list))
            .nest("/admin", admin_routes);

        self
    }

    pub async fn with_auth_service(mut self, auth_uri: &Uri) -> Self {
        let jwt_cache_manager: Arc<Box<dyn CacheManagement<Value = String>>> =
            Arc::new(Box::new(CacheManager::new(1000)));

        self.auth_cache = Some(jwt_cache_manager.clone());

        let auth_client = shuttle_proto::auth::client(auth_uri)
            .await
            .expect("auth service should be reachable");

        let auth_public_key = AuthPublicKey::new(auth_client.clone());

        self.auth_client = Some(auth_client.clone());

        self.router = self
            .router
            .layer(JwtAuthenticationLayer::new(auth_public_key))
            .layer(ShuttleAuthLayer::new(jwt_cache_manager, auth_client))
            // These routes expect an api-key bearer token, which would be converted to a JWT if
            // it was passed through the auth layer.
            .route("/login/:account_name", post(login))
            .route("/users/:account_name", get(get_user))
            .route("/users/:account_name/:account_tier", post(post_user))
            .route("/users/reset-api-key", put(reset_api_key));

        self
    }

    pub fn into_router(self) -> Router {
        let service = self.service.expect("a GatewayService is required");
        let auth_cache = self.auth_cache.expect("an auth cache is required");
        let auth_client = self.auth_client.expect("an auth client is required");

        self.router.with_state(RouterState {
            auth_cache,
            auth_client,
            service,
        })
    }

    pub fn serve(self) -> impl Future<Output = Result<(), hyper::Error>> {
        let bind = self.bind.expect("a socket address to bind to is required");
        let router = self.into_router();
        axum::Server::bind(&bind).serve(router.into_make_service())
    }
}

#[cfg(test)]
pub mod tests {
    use axum::body::Body;
    use axum::headers::Authorization;
    use axum::http::Request;
    use axum_extra::extract::cookie;
    use futures::TryFutureExt;
    use http::HeaderValue;
    use hyper::StatusCode;
    use serde_json::Value;
    use tower::Service;

    use super::*;
    use crate::service::GatewayService;
    use crate::tests::{RequestBuilderExt, World};

    #[tokio::test]
    async fn api_auth_endpoints() -> anyhow::Result<()> {
        use rand::distributions::{Alphanumeric, DistString};

        // The api key of the admin user we inserted when setting up the shuttle environment,
        // see the contribution doc for how to do that.
        const AUTH_ADMIN_KEY: &str = "dh9z58jttoes3qvt";
        let admin_authorization = Authorization::bearer(AUTH_ADMIN_KEY).unwrap();

        let world = World::new().await;
        let service = Arc::new(GatewayService::init(world.pool(), "".into(), world.fqdn()).await);

        // The address of the auth service we start with `make up`, see the contribution doc.
        let auth_uri: Uri = "http://127.0.0.1:8008".parse().unwrap();

        let mut router = ApiBuilder::new()
            .with_service(service)
            .with_default_routes()
            .with_auth_service(&auth_uri)
            .await
            .into_router();

        // We'll insert a new user for our tests, first generate a name.
        let new_user_name = Alphanumeric.sample_string(&mut rand::thread_rng(), 10);

        let post_user_request = || {
            Request::builder()
                .method("POST")
                .uri(format!("/users/{new_user_name}/basic"))
                .body(Body::empty())
                .unwrap()
        };

        // Post user without admin user.
        router
            .call(post_user_request())
            .map_ok(|resp| assert_eq!(resp.status(), StatusCode::UNAUTHORIZED))
            .await
            .unwrap();

        // Post user with admin token.
        router
            .call(post_user_request().with_header(&admin_authorization))
            .map_ok(|resp| assert_eq!(resp.status(), StatusCode::OK))
            .await
            .unwrap();

        // Login without admin user bearer token.
        let login_request = || {
            Request::builder()
                .method("POST")
                .uri(format!("/login/{new_user_name}"))
                .body(Body::empty())
                .unwrap()
        };

        router
            .call(login_request())
            .map_ok(|resp| assert_eq!(resp.status(), StatusCode::UNAUTHORIZED))
            .await
            .unwrap();

        // Login with admin user and verify that it returns the expected cookie.
        let login_response = router
            .call(login_request().with_header(&admin_authorization))
            .await
            .unwrap();

        let cookie = login_response
            .headers()
            .get("set-cookie")
            .unwrap()
            .to_str()
            .unwrap();

        let cookie = Cookie::parse(cookie).unwrap();

        assert_eq!(cookie.http_only(), Some(true));
        assert_eq!(cookie.same_site(), Some(cookie::SameSite::Strict));
        assert_eq!(cookie.secure(), Some(true));
        assert_eq!(cookie.name(), COOKIE_NAME);

        let get_user_request = || {
            Request::builder()
                .method("GET")
                .uri(format!("/users/{new_user_name}"))
                .body(Body::empty())
                .unwrap()
        };

        // Get user without admin user bearer token.
        router
            .call(get_user_request())
            .map_ok(|resp| assert_eq!(resp.status(), StatusCode::UNAUTHORIZED))
            .await
            .unwrap();

        // Get user with admin user bearer token.
        let user = router
            .call(get_user_request().with_header(&admin_authorization))
            .await
            .unwrap();

        assert_eq!(user.status(), StatusCode::OK);
        let user: Value =
            serde_json::from_slice(&hyper::body::to_bytes(user.into_body()).await.unwrap())
                .unwrap();

        let user_api_key = user["key"].as_str().unwrap();

        let reset_key_request = || {
            Request::builder()
                .method("PUT")
                .uri("/users/reset-api-key")
                .body(Body::empty())
                .unwrap()
        };

        // Reset the api-key of our test using their api-key.
        router
            .call(reset_key_request().with_header(&Authorization::bearer(user_api_key).unwrap()))
            .map_ok(|resp| assert_eq!(resp.status(), StatusCode::OK))
            .await
            .unwrap();

        // Reset the api-key of our test user using the cookie from login.
        let mut reset_key_request = reset_key_request();

        reset_key_request
            .headers_mut()
            .insert(COOKIE, HeaderValue::from_str(&cookie.to_string()).unwrap());

        router
            .call(reset_key_request)
            .map_ok(|resp| assert_eq!(resp.status(), StatusCode::OK))
            .await
            .unwrap();

        // Logout our test user and verify that it returns the expected removal cookie,
        // this expects the shuttle.sid cookie to be set know which user to logout.
        let mut logout_request = Request::builder()
            .method("POST")
            .uri("/logout")
            .body(Body::empty())
            .unwrap();

        logout_request
            .headers_mut()
            .insert(COOKIE, HeaderValue::from_str(&cookie.to_string()).unwrap());

        let logout_response = router.call(logout_request).await.unwrap();

        let cookie = logout_response
            .headers()
            .get("set-cookie")
            .unwrap()
            .to_str()
            .unwrap();

        let cookie = Cookie::parse(cookie).unwrap();

        assert_eq!(cookie.http_only(), Some(true));
        assert_eq!(cookie.name(), COOKIE_NAME);

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn status() {
        let world = World::new().await;
        let service = Arc::new(GatewayService::init(world.pool(), "".into(), world.fqdn()).await);

        let mut router = ApiBuilder::new()
            .with_service(service)
            .with_default_routes()
            .with_auth_service(&world.context().auth_uri)
            .await
            .into_router();

        let get_status = || {
            Request::builder()
                .method("GET")
                .uri("/")
                .body(Body::empty())
                .unwrap()
        };

        let resp = router.call(get_status()).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }
}
