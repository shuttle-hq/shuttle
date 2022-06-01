use std::sync::Arc;

use axum::body::Body;
use axum::extract::{
    Extension,
    Path
};
use axum::http::{
    Request,
    StatusCode
};
use axum::response::Response;
use axum::routing::{
    any,
    get
};
use axum::{
    Json as AxumJson,
    Router
};

use crate::{Error, ProjectName, auth::{User, ScopedUser}, AccountName};
use crate::project::Project;
use crate::GatewayService;
use crate::auth::Admin;

async fn get_user(
    Extension(service): Extension<Arc<GatewayService>>,
    Path(account_name): Path<AccountName>,
    _: Admin
) -> Result<AxumJson<User>, StatusCode> {
    service
        .user_from_account_name(account_name)
        .await
        .map(|user| AxumJson(user))
        .map_err(|_| StatusCode::NOT_FOUND)
}

async fn post_user(
    Extension(service): Extension<Arc<GatewayService>>,
    Path(account_name): Path<AccountName>,
    _: Admin
) -> Result<AxumJson<User>, StatusCode> {
    service
        .create_user(account_name)
        .await
        .map(|user| AxumJson(user))
        .map_err(|_| StatusCode::BAD_REQUEST)
}

async fn get_project(
    Extension(service): Extension<Arc<GatewayService>>,
    ScopedUser { scope, .. }: ScopedUser
) -> Result<AxumJson<Project>, StatusCode> {
    service
        .find_project(&scope)
        .await
        .map(|project| AxumJson(project))
        .ok_or_else(|| StatusCode::NOT_FOUND)
}

async fn post_project(
    Extension(service): Extension<Arc<GatewayService>>,
    User { name, .. }: User,
    Path(project): Path<ProjectName>
) -> Result<AxumJson<Project>, Error> {
    service
        .create_project(project, name)
        .await
        .map(|project| AxumJson(project))
}

async fn delete_project(Path(_project): Path<String>) {
    todo!()
}

async fn route_project(
    ScopedUser { scope, .. }: ScopedUser,
    Extension(service): Extension<Arc<GatewayService>>,
    Path((_, route)): Path<(String, String)>,
    req: Request<Body>
) -> Response<Body> {
    service.route(&scope, route, req).await.unwrap()
}

pub fn make_api(service: Arc<GatewayService>) -> Router<Body> {
    Router::<Body>::new()
        .route(
            "/projects/:project",
            get(get_project).delete(delete_project).post(post_project)
        )
        .route(
            "/users/:account_name",
            get(get_user).post(post_user)
        )
        .route("/projects/:project/*any", any(route_project))
        .layer(Extension(service))
}
