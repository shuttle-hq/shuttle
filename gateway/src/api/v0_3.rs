//! # Compatibility layer with v0.3.x client and deployer APIs

use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Extension, FromRequest, Path, RequestParts};
use axum::headers::authorization::Basic;
use axum::headers::{Authorization, Header, HeaderMapExt};
use axum::http::Request;
use axum::response::Response;
use axum::routing::get;
use axum::Router;
use shuttle_common::ApiKey;

use crate::auth::{Admin, ScopedUser, User};
use crate::{AccountName, Error, ErrorKind, GatewayService, ProjectName};

/// A request guard that flips the basic authorization header so that
/// the API key (which is the username field in `v0.3`) now is in the
/// password field
pub struct BackwardAuth<U>(pub U);

#[async_trait]
impl<B, U> FromRequest<B> for BackwardAuth<U>
where
    U: FromRequest<B>,
    B: Send,
{
    type Rejection = U::Rejection;

    async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
        if let Some(header) = req.headers_mut().typed_get::<Authorization<Basic>>() {
            req.headers_mut().remove(Authorization::<Basic>::name());
            let username = header.0.username();
            let header = Authorization::<Basic>::basic("", username);
            req.headers_mut().typed_insert(header);
        }
        Ok(Self(U::from_request(req).await?))
    }
}

async fn get_user(
    Extension(service): Extension<Arc<GatewayService>>,
    Path(account_name): Path<AccountName>,
    _: Admin,
) -> Result<ApiKey, Error> {
    service
        .user_from_account_name(account_name)
        .await
        .map(|user| user.key.0)
}

async fn post_user(
    Extension(service): Extension<Arc<GatewayService>>,
    Path(account_name): Path<AccountName>,
    _: Admin,
) -> Result<ApiKey, Error> {
    service
        .create_user(account_name)
        .await
        .map(|user| user.key.0)
}

async fn get_project(
    Extension(service): Extension<Arc<GatewayService>>,
    Path(project): Path<ProjectName>,
    req: Request<Body>,
    BackwardAuth(user): BackwardAuth<User>,
) -> Result<Response<Body>, Error> {
    if !user.projects.contains(&project) {
        return Err(Error::from_kind(ErrorKind::Forbidden));
    }
    route_project(
        Extension(service),
        ScopedUser {
            user,
            scope: project.clone(),
        },
        Path((project, String::default())),
        req,
    )
    .await
}

/// The `v0.3 -> v0.4` layer for this does two things:
/// 1. Check if the project already exists - and otherwise creates it
/// 2. Pass the request, untouched, to the created project runtime
///    when it's ready
async fn post_project(
    Extension(service): Extension<Arc<GatewayService>>,
    BackwardAuth(user): BackwardAuth<User>,
    Path(project): Path<ProjectName>,
    req: Request<Body>,
) -> Result<Response<Body>, Error> {
    match service.find_project(&project).await {
        Err(err) if err.kind() == ErrorKind::ProjectNotFound => {
            service
                .create_project(project.clone(), user.name.clone())
                .await?;
        }
        Err(err) => return Err(err),
        Ok(_) => {
            if !user.projects.contains(&project) {
                return Err(Error::from_kind(ErrorKind::Forbidden));
            }
        }
    };
    route_project(
        Extension(service),
        ScopedUser {
            user,
            scope: project.clone(),
        },
        Path((project, String::default())),
        req,
    )
    .await
}

async fn delete_project(
    Extension(service): Extension<Arc<GatewayService>>,
    BackwardAuth(user): BackwardAuth<User>,
    Path(project): Path<ProjectName>,
    req: Request<Body>,
) -> Result<Response<Body>, Error> {
    if !user.projects.contains(&project) {
        return Err(Error::from_kind(ErrorKind::Forbidden));
    }
    route_project(
        Extension(service),
        ScopedUser {
            user,
            scope: project.clone(),
        },
        Path((project, String::default())),
        req,
    )
    .await
}

async fn route_project(
    Extension(service): Extension<Arc<GatewayService>>,
    ScopedUser { scope, .. }: ScopedUser,
    Path((project, route)): Path<(ProjectName, String)>,
    req: Request<Body>,
) -> Result<Response<Body>, Error> {
    // All routes in the `v0.3` deployer are prefixed by
    // `/projects/{project}`
    let route = format!("/projects/{project}/{route}");
    service.route(&scope, Path(route), req).await
}

pub fn make_api(service: Arc<GatewayService>) -> Router<Body> {
    Router::<Body>::new()
        .route(
            "/projects/:project",
            get(get_project).delete(delete_project).post(post_project),
        )
        .route("/users/:account_name", get(get_user).post(post_user))
        .layer(Extension(service))
}
