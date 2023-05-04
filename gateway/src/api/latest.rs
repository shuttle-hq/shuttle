use std::io::Cursor;
use std::net::SocketAddr;
use std::ops::Sub;
use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::extract::{Extension, Path, Query, State};
use axum::handler::Handler;
use axum::http::Request;
use axum::middleware::from_extractor;
use axum::response::Response;
use axum::routing::{any, get, post};
use axum::{Json as AxumJson, Router};
use fqdn::FQDN;
use futures::Future;
use http::{StatusCode, Uri};
use instant_acme::{AccountCredentials, ChallengeType};
use serde::{Deserialize, Serialize};
use shuttle_common::backends::auth::{AuthPublicKey, JwtAuthenticationLayer, ScopedLayer};
use shuttle_common::backends::cache::CacheManager;
use shuttle_common::backends::metrics::{Metrics, TraceLayer};
use shuttle_common::claims::{Scope, EXP_MINUTES};
use shuttle_common::models::error::ErrorKind;
use shuttle_common::models::{project, stats};
use shuttle_common::request_span;
use tokio::sync::mpsc::Sender;
use tokio::sync::{Mutex, MutexGuard};
use tracing::{field, instrument, trace};
use ttl_cache::TtlCache;

use utoipa::openapi::security::{ApiKey, ApiKeyValue, SecurityScheme};
use utoipa::{IntoParams, Modify, OpenApi};
use utoipa_swagger_ui::SwaggerUi;
use uuid::Uuid;
use x509_parser::nom::AsBytes;
use x509_parser::parse_x509_certificate;
use x509_parser::pem::parse_x509_pem;
use x509_parser::time::ASN1Time;

use crate::acme::{AcmeClient, CustomDomain};
use crate::auth::{ScopedUser, User};
use crate::project::{ContainerInspectResponseExt, Project, ProjectCreating};
use crate::service::GatewayService;
use crate::task::{self, BoxedTask, TaskResult};
use crate::tls::{GatewayCertResolver, RENEWAL_VALIDITY_THRESHOLD_IN_DAYS};
use crate::worker::WORKER_QUEUE_SIZE;
use crate::{Error, ProjectName};

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

#[derive(Debug, Clone, Copy, Deserialize, IntoParams)]
struct PaginationDetails {
    /// Page to fetch, starting from 0.
    page: Option<u32>,
    /// Number of results per page.
    limit: Option<u32>,
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
async fn get_project(
    State(RouterState { service, .. }): State<RouterState>,
    ScopedUser { scope, .. }: ScopedUser,
) -> Result<AxumJson<project::Response>, Error> {
    let state = service.find_project(&scope).await?.into();
    let response = project::Response {
        name: scope.to_string(),
        state,
    };

    Ok(AxumJson(response))
}

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
async fn get_projects_list(
    State(RouterState { service, .. }): State<RouterState>,
    User { name, .. }: User,
    Query(PaginationDetails { page, limit }): Query<PaginationDetails>,
) -> Result<AxumJson<Vec<project::Response>>, Error> {
    let limit = limit.unwrap_or(u32::MAX);
    let page = page.unwrap_or(0);
    let projects = service
        // The `offset` is page size * amount of pages
        .iter_user_projects_detailed(&name, limit * page, limit)
        .await?
        .map(|project| project::Response {
            name: project.0.to_string(),
            state: project.1.into(),
        })
        .collect();

    Ok(AxumJson(projects))
}

#[instrument(skip_all, fields(%project))]
#[utoipa::path(
    post,
    path = "/projects/{project_name}",
    responses(
        (status = 200, description = "Successfully created a specific project.", body = shuttle_common::models::project::Response),
        (status = 500, description = "Server internal error.")
    ),
    params(
        ("project_name" = String, Path, description = "The name of the project."),
    )
)]
async fn create_project(
    State(RouterState {
        service, sender, ..
    }): State<RouterState>,
    User { name, claim, .. }: User,
    Path(project): Path<ProjectName>,
    AxumJson(config): AxumJson<project::Config>,
) -> Result<AxumJson<project::Response>, Error> {
    let is_admin = claim.scopes.contains(&Scope::Admin);

    let state = service
        .create_project(project.clone(), name.clone(), is_admin, config.idle_minutes)
        .await?;

    service
        .new_task()
        .project(project.clone())
        .send(&sender)
        .await?;

    let response = project::Response {
        name: project.to_string(),
        state: state.into(),
    };

    Ok(AxumJson(response))
}

#[instrument(skip_all, fields(%project))]
#[utoipa::path(
    delete,
    path = "/projects/{project_name}",
    responses(
        (status = 200, description = "Successfully destroyed a specific project.", body = shuttle_common::models::project::Response),
        (status = 500, description = "Server internal error.")
    ),
    params(
        ("project_name" = String, Path, description = "The name of the project."),
    )
)]
async fn destroy_project(
    State(RouterState {
        service, sender, ..
    }): State<RouterState>,
    ScopedUser { scope: project, .. }: ScopedUser,
) -> Result<AxumJson<project::Response>, Error> {
    let state = service.find_project(&project).await?;

    let mut response = project::Response {
        name: project.to_string(),
        state: state.into(),
    };

    if response.state == shuttle_common::models::project::State::Destroyed {
        return Ok(AxumJson(response));
    }

    // if project exists and isn't `Destroyed`, send destroy task
    service
        .new_task()
        .project(project)
        .and_then(task::destroy())
        .send(&sender)
        .await?;

    response.state = shuttle_common::models::project::State::Destroying;

    Ok(AxumJson(response))
}

#[instrument(skip_all, fields(scope = %scoped_user.scope))]
async fn route_project(
    State(RouterState {
        service, sender, ..
    }): State<RouterState>,
    scoped_user: ScopedUser,
    req: Request<Body>,
) -> Result<Response<Body>, Error> {
    let project_name = scoped_user.scope;
    let project = service.find_or_start_project(&project_name, sender).await?;

    service
        .route(&project, &project_name, &scoped_user.user.name, req)
        .await
}

#[utoipa::path(
    get,
    path = "/",
    responses(
        (status = 200, description = "Get the gateway operational status."),
        (status = 500, description = "Server internal error.")
    )
)]
async fn get_status(State(RouterState { sender, .. }): State<RouterState>) -> Response<Body> {
    let (status, body) = if sender.is_closed() || sender.capacity() == 0 {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            StatusResponse::unhealthy(),
        )
    } else if sender.capacity() < WORKER_QUEUE_SIZE - SVC_DEGRADED_THRESHOLD {
        (StatusCode::OK, StatusResponse::degraded())
    } else {
        (StatusCode::OK, StatusResponse::healthy())
    };

    let body = serde_json::to_vec(&body).unwrap();
    Response::builder()
        .status(status)
        .body(body.into())
        .unwrap()
}

#[instrument(skip_all)]
#[utoipa::path(
    post,
    path = "/stats/load",
    responses(
        (status = 200, description = "Successfully fetched the build queue load.", body = shuttle_common::models::stats::LoadResponse),
        (status = 500, description = "Server internal error.")
    )
)]
async fn post_load(
    State(RouterState { running_builds, .. }): State<RouterState>,
    AxumJson(build): AxumJson<stats::LoadRequest>,
) -> Result<AxumJson<stats::LoadResponse>, Error> {
    let mut running_builds = running_builds.lock().await;

    trace!(id = %build.id, "checking build queue");
    let mut load = calculate_capacity(&mut running_builds);

    if load.has_capacity
        && running_builds
            .insert(build.id, (), Duration::from_secs(60 * EXP_MINUTES as u64))
            .is_none()
    {
        // Only increase when an item was not already in the queue
        load.builds_count += 1;
    }

    Ok(AxumJson(load))
}

#[instrument(skip_all)]
#[utoipa::path(
    delete,
    path = "/stats/load",
    responses(
        (status = 200, description = "Successfully removed the build with the ID specified in the load request from the build queue.", body = shuttle_common::models::stats::LoadResponse),
        (status = 500, description = "Server internal error.")
    )
)]
async fn delete_load(
    State(RouterState { running_builds, .. }): State<RouterState>,
    AxumJson(build): AxumJson<stats::LoadRequest>,
) -> Result<AxumJson<stats::LoadResponse>, Error> {
    let mut running_builds = running_builds.lock().await;
    running_builds.remove(&build.id);

    trace!(id = %build.id, "removing from build queue");
    let load = calculate_capacity(&mut running_builds);

    Ok(AxumJson(load))
}

#[instrument(skip_all)]
#[utoipa::path(
    get,
    path = "/admin/stats/load",
    responses(
        (status = 200, description = "Successfully gets the build queue load as an admin.", body = shuttle_common::models::stats::LoadResponse),
        (status = 500, description = "Server internal error.")
    )
)]
async fn get_load_admin(
    State(RouterState { running_builds, .. }): State<RouterState>,
) -> Result<AxumJson<stats::LoadResponse>, Error> {
    let mut running_builds = running_builds.lock().await;

    let load = calculate_capacity(&mut running_builds);

    Ok(AxumJson(load))
}

#[instrument(skip_all)]
#[utoipa::path(
    delete,
    path = "/admin/stats/load",
    responses(
        (status = 200, description = "Successfully clears the build queue.", body = shuttle_common::models::stats::LoadResponse),
        (status = 500, description = "Server internal error.")
    )
)]
async fn delete_load_admin(
    State(RouterState { running_builds, .. }): State<RouterState>,
) -> Result<AxumJson<stats::LoadResponse>, Error> {
    let mut running_builds = running_builds.lock().await;
    running_builds.clear();

    let load = calculate_capacity(&mut running_builds);

    Ok(AxumJson(load))
}

fn calculate_capacity(running_builds: &mut MutexGuard<TtlCache<Uuid, ()>>) -> stats::LoadResponse {
    let active = running_builds.iter().count();
    let capacity = running_builds.capacity();
    let has_capacity = active < capacity;

    stats::LoadResponse {
        builds_count: active,
        has_capacity,
    }
}

#[instrument(skip_all)]
#[utoipa::path(
    post,
    path = "/admin/revive",
    responses(
        (status = 200, description = "Successfully revived stopped or errored projects."),
        (status = 500, description = "Server internal error.")
    )
)]
async fn revive_projects(
    State(RouterState {
        service, sender, ..
    }): State<RouterState>,
) -> Result<(), Error> {
    crate::project::exec::revive(service, sender)
        .await
        .map_err(|_| Error::from_kind(ErrorKind::Internal))
}

#[instrument(skip_all)]
#[utoipa::path(
    post,
    path = "/admin/destroy",
    responses(
        (status = 200, description = "Successfully destroyed the projects."),
        (status = 500, description = "Server internal error.")
    )
)]
async fn destroy_projects(
    State(RouterState {
        service, sender, ..
    }): State<RouterState>,
) -> Result<(), Error> {
    crate::project::exec::destroy(service, sender)
        .await
        .map_err(|_| Error::from_kind(ErrorKind::Internal))
}

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
async fn request_custom_domain_acme_certificate(
    State(RouterState {
        service, sender, ..
    }): State<RouterState>,
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

    let project = service.find_project(&project_name).await?;
    let idle_minutes = project.container().unwrap().idle_minutes();

    // Destroy and recreate the project with the new domain.
    service
        .new_task()
        .project(project_name.clone())
        .and_then(task::destroy())
        .and_then(task::run_until_done())
        .and_then(task::run({
            let fqdn = fqdn.to_string();
            move |ctx| {
                let fqdn = fqdn.clone();
                async move {
                    let creating = ProjectCreating::new_with_random_initial_key(
                        ctx.project_name,
                        idle_minutes,
                    )
                    .with_fqdn(fqdn);
                    TaskResult::Done(Project::Creating(creating))
                }
            }
        }))
        .send(&sender)
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
async fn renew_custom_domain_acme_certificate(
    State(RouterState { service, .. }): State<RouterState>,
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
async fn renew_gateway_acme_certificate(
    State(RouterState { service, .. }): State<RouterState>,
    Extension(acme_client): Extension<AcmeClient>,
    Extension(resolver): Extension<Arc<GatewayCertResolver>>,
    AxumJson(credentials): AxumJson<AccountCredentials<'_>>,
) -> Result<String, Error> {
    service
        .renew_certificate(&acme_client, resolver, credentials)
        .await;
    Ok(r#""Renewed the gateway certificate.""#.to_string())
}

#[utoipa::path(
    post,
    path = "/admin/projects",
    responses(
        (status = 200, description = "Successfully fetched the projects list.", body = shuttle_common::models::project::AdminResponse),
        (status = 500, description = "Server internal error.")
    )
)]
async fn get_projects(
    State(RouterState { service, .. }): State<RouterState>,
) -> Result<AxumJson<Vec<project::AdminResponse>>, Error> {
    let projects = service
        .iter_projects_detailed()
        .await?
        .map(Into::into)
        .collect();

    Ok(AxumJson(projects))
}

struct SecurityAddon;

impl Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                "Gateway API Key",
                SecurityScheme::ApiKey(ApiKey::Header(ApiKeyValue::new("Bearer"))),
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
        get_status,
        get_projects_list,
        get_project,
        destroy_project,
        create_project,
        post_load,
        delete_load,
        get_projects,
        revive_projects,
        destroy_projects,
        get_load_admin,
        delete_load_admin
    ),
    modifiers(&SecurityAddon),
    components(schemas(
        shuttle_common::models::project::Response,
        shuttle_common::models::stats::LoadResponse,
        shuttle_common::models::project::AdminResponse,
        shuttle_common::models::stats::LoadResponse,
        shuttle_common::models::project::State
    ))
)]
pub struct ApiDoc;

#[derive(Clone)]
pub(crate) struct RouterState {
    pub service: Arc<GatewayService>,
    pub sender: Sender<BoxedTask>,
    pub running_builds: Arc<Mutex<TtlCache<Uuid, ()>>>,
}

pub struct ApiBuilder {
    router: Router<RouterState>,
    service: Option<Arc<GatewayService>>,
    sender: Option<Sender<BoxedTask>>,
    bind: Option<SocketAddr>,
}

impl Default for ApiBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl ApiBuilder {
    pub fn new() -> Self {
        Self {
            router: Router::new(),
            service: None,
            sender: None,
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

    pub fn with_service(mut self, service: Arc<GatewayService>) -> Self {
        self.service = Some(service);
        self
    }

    pub fn with_sender(mut self, sender: Sender<BoxedTask>) -> Self {
        self.sender = Some(sender);
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
            .route("/revive", post(revive_projects))
            .route("/destroy", post(destroy_projects))
            .route("/stats/load", get(get_load_admin).delete(delete_load_admin))
            // TODO: The `/swagger-ui` responds with a 303 See Other response which is followed in
            // browsers but leads to 404 Not Found. This must be investigated.
            .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
            .layer(ScopedLayer::new(vec![Scope::Admin]));

        self.router = self
            .router
            .route("/", get(get_status))
            .route(
                "/projects",
                get(get_projects_list.layer(ScopedLayer::new(vec![Scope::Project]))),
            )
            .route(
                "/projects/:project_name",
                get(get_project.layer(ScopedLayer::new(vec![Scope::Project])))
                    .delete(destroy_project.layer(ScopedLayer::new(vec![Scope::ProjectCreate])))
                    .post(create_project.layer(ScopedLayer::new(vec![Scope::ProjectCreate]))),
            )
            .route("/projects/:project_name/*any", any(route_project))
            .route("/stats/load", post(post_load).delete(delete_load))
            .nest("/admin", admin_routes);

        self
    }

    pub fn with_auth_service(mut self, auth_uri: Uri) -> Self {
        let auth_public_key = AuthPublicKey::new(auth_uri.clone());

        let jwt_cache_manager = CacheManager::new(1000);

        self.router = self
            .router
            .layer(JwtAuthenticationLayer::new(auth_public_key))
            .layer(ShuttleAuthLayer::new(
                auth_uri,
                Arc::new(Box::new(jwt_cache_manager)),
            ));

        self
    }

    pub fn into_router(self) -> Router {
        let service = self.service.expect("a GatewayService is required");
        let sender = self.sender.expect("a task Sender is required");

        // Allow about 4 cores per build
        let mut concurrent_builds = num_cpus::get() / 4;
        if concurrent_builds < 1 {
            concurrent_builds = 1;
        }

        let running_builds = Arc::new(Mutex::new(TtlCache::new(concurrent_builds)));

        self.router.with_state(RouterState {
            service,
            sender,
            running_builds,
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
    use std::sync::Arc;

    use axum::body::Body;
    use axum::headers::Authorization;
    use axum::http::Request;
    use futures::TryFutureExt;
    use hyper::StatusCode;
    use tokio::sync::mpsc::channel;
    use tokio::sync::oneshot;
    use tower::Service;

    use super::*;
    use crate::service::GatewayService;
    use crate::tests::{RequestBuilderExt, World};

    #[tokio::test]
    async fn api_create_get_delete_projects() -> anyhow::Result<()> {
        let world = World::new().await;
        let service = Arc::new(GatewayService::init(world.args(), world.pool(), "".into()).await);

        let (sender, mut receiver) = channel::<BoxedTask>(256);
        tokio::spawn(async move {
            while receiver.recv().await.is_some() {
                // do not do any work with inbound requests
            }
        });

        let mut router = ApiBuilder::new()
            .with_service(Arc::clone(&service))
            .with_sender(sender)
            .with_default_routes()
            .with_auth_service(world.context().auth_uri)
            .into_router();

        let neo_key = world.create_user("neo");

        let create_project = |project: &str| {
            Request::builder()
                .method("POST")
                .uri(format!("/projects/{project}"))
                .header("Content-Type", "application/json")
                .body("{\"idle_minutes\": 3}".into())
                .unwrap()
        };

        let delete_project = |project: &str| {
            Request::builder()
                .method("DELETE")
                .uri(format!("/projects/{project}"))
                .body(Body::empty())
                .unwrap()
        };

        router
            .call(create_project("matrix"))
            .map_ok(|resp| assert_eq!(resp.status(), StatusCode::UNAUTHORIZED))
            .await
            .unwrap();

        let authorization = Authorization::bearer(&neo_key).unwrap();

        router
            .call(create_project("matrix").with_header(&authorization))
            .map_ok(|resp| {
                assert_eq!(resp.status(), StatusCode::OK);
            })
            .await
            .unwrap();

        router
            .call(create_project("matrix").with_header(&authorization))
            .map_ok(|resp| {
                assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
            })
            .await
            .unwrap();

        let get_project = |project| {
            Request::builder()
                .method("GET")
                .uri(format!("/projects/{project}"))
                .body(Body::empty())
                .unwrap()
        };

        router
            .call(get_project("matrix"))
            .map_ok(|resp| {
                assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
            })
            .await
            .unwrap();

        router
            .call(get_project("matrix").with_header(&authorization))
            .map_ok(|resp| {
                assert_eq!(resp.status(), StatusCode::OK);
            })
            .await
            .unwrap();

        router
            .call(delete_project("matrix").with_header(&authorization))
            .map_ok(|resp| {
                assert_eq!(resp.status(), StatusCode::OK);
            })
            .await
            .unwrap();

        router
            .call(create_project("reloaded").with_header(&authorization))
            .map_ok(|resp| {
                assert_eq!(resp.status(), StatusCode::OK);
            })
            .await
            .unwrap();

        let trinity_key = world.create_user("trinity");

        let authorization = Authorization::bearer(&trinity_key).unwrap();

        router
            .call(get_project("reloaded").with_header(&authorization))
            .map_ok(|resp| assert_eq!(resp.status(), StatusCode::NOT_FOUND))
            .await
            .unwrap();

        router
            .call(delete_project("reloaded").with_header(&authorization))
            .map_ok(|resp| {
                assert_eq!(resp.status(), StatusCode::NOT_FOUND);
            })
            .await
            .unwrap();

        let get_load = || {
            Request::builder()
                .method("GET")
                .uri("/admin/stats/load")
                .body(Body::empty())
                .unwrap()
        };

        // Non-admin user cannot access admin routes
        router
            .call(get_load().with_header(&authorization))
            .map_ok(|resp| {
                assert_eq!(resp.status(), StatusCode::FORBIDDEN);
            })
            .await
            .unwrap();

        // Create new admin user
        let admin_neo_key = world.create_user("admin-neo");
        world.set_super_user("admin-neo");

        let authorization = Authorization::bearer(&admin_neo_key).unwrap();

        // Admin user can access admin routes
        router
            .call(get_load().with_header(&authorization))
            .map_ok(|resp| {
                assert_eq!(resp.status(), StatusCode::OK);
            })
            .await
            .unwrap();

        // TODO: setting the user to admin here doesn't update the cached token, so the
        // commands will still fail. We need to add functionality for this or modify the test.
        // world.set_super_user("trinity");

        // router
        //     .call(get_project("reloaded").with_header(&authorization))
        //     .map_ok(|resp| assert_eq!(resp.status(), StatusCode::OK))
        //     .await
        //     .unwrap();

        // router
        //     .call(delete_project("reloaded").with_header(&authorization))
        //     .map_ok(|resp| {
        //         assert_eq!(resp.status(), StatusCode::OK);
        //     })
        //     .await
        //     .unwrap();

        // // delete returns 404 for project that doesn't exist
        // router
        //     .call(delete_project("resurrections").with_header(&authorization))
        //     .map_ok(|resp| {
        //         assert_eq!(resp.status(), StatusCode::NOT_FOUND);
        //     })
        //     .await
        //     .unwrap();

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn status() {
        let world = World::new().await;
        let service = Arc::new(GatewayService::init(world.args(), world.pool(), "".into()).await);

        let (sender, mut receiver) = channel::<BoxedTask>(1);
        let (ctl_send, ctl_recv) = oneshot::channel();
        let (done_send, done_recv) = oneshot::channel();
        let worker = tokio::spawn(async move {
            let mut done_send = Some(done_send);
            // do not process until instructed
            ctl_recv.await.unwrap();

            while receiver.recv().await.is_some() {
                done_send.take().unwrap().send(()).unwrap();
                // do nothing
            }
        });

        let mut router = ApiBuilder::new()
            .with_service(Arc::clone(&service))
            .with_sender(sender)
            .with_default_routes()
            .with_auth_service(world.context().auth_uri)
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

        let matrix: ProjectName = "matrix".parse().unwrap();

        let neo_key = world.create_user("neo");
        let authorization = Authorization::bearer(&neo_key).unwrap();

        let create_project = Request::builder()
            .method("POST")
            .uri(format!("/projects/{matrix}"))
            .header("Content-Type", "application/json")
            .body("{\"idle_minutes\": 3}".into())
            .unwrap()
            .with_header(&authorization);

        router.call(create_project).await.unwrap();

        let resp = router.call(get_status()).await.unwrap();
        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);

        ctl_send.send(()).unwrap();
        done_recv.await.unwrap();

        let resp = router.call(get_status()).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        worker.abort();
        let _ = worker.await;

        let resp = router.call(get_status()).await.unwrap();
        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }
}
