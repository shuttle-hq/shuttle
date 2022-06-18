use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Extension, Path};
use axum::http::{Request, StatusCode};
use axum::response::Response;
use axum::routing::{any, get};
use axum::{Json as AxumJson, Router};

use crate::auth::Admin;
use crate::project::Project;
use crate::{
    auth::{ScopedUser, User},
    AccountName, Error, ProjectName,
};
use crate::{ErrorKind, GatewayService};

async fn get_user(
    Extension(service): Extension<Arc<GatewayService>>,
    Path(account_name): Path<AccountName>,
    _: Admin,
) -> Result<AxumJson<User>, Error> {
    service
        .user_from_account_name(account_name)
        .await
        .map(AxumJson)
}

async fn post_user(
    Extension(service): Extension<Arc<GatewayService>>,
    Path(account_name): Path<AccountName>,
    _: Admin,
) -> Result<AxumJson<User>, Error> {
    service
        .create_user(account_name)
        .await
        .map(AxumJson)
}

async fn get_project(
    Extension(service): Extension<Arc<GatewayService>>,
    ScopedUser { scope, .. }: ScopedUser,
) -> Result<AxumJson<Project>, Error> {
    service
        .find_project(&scope)
        .await
        .map(AxumJson)
}

async fn post_project(
    Extension(service): Extension<Arc<GatewayService>>,
    User { name, .. }: User,
    Path(project): Path<ProjectName>,
) -> Result<AxumJson<Project>, Error> {
    service
        .create_project(project, name)
        .await
        .map(AxumJson)
}

async fn delete_project(
    Extension(service): Extension<Arc<GatewayService>>,
    User { name, .. }: User,
    Path(project): Path<ProjectName>,
) -> Result<(), Error> {
    service
        .destroy_project(project, name)
        .await
}

async fn route_project(
    ScopedUser { scope, .. }: ScopedUser,
    Extension(service): Extension<Arc<GatewayService>>,
    Path((_, route)): Path<(String, String)>,
    req: Request<Body>,
) -> Response<Body> {
    service.route(&scope, route, req).await.unwrap()
}

pub fn make_api(service: Arc<GatewayService>) -> Router<Body> {
    Router::<Body>::new()
        .route(
            "/projects/:project",
            get(get_project).delete(delete_project).post(post_project),
        )
        .route("/users/:account_name", get(get_user).post(post_user))
        .route("/projects/:project/*any", any(route_project))
        .layer(Extension(service))
}

#[cfg(test)]
pub mod tests {
    use std::sync::Arc;

    use tokio::sync::mpsc::channel;

    use tower::Service;
    use axum::{body::{Body, HttpBody}, http::Request, headers::{Header, Authorization, authorization::Basic}};

    use super::*;

    use crate::{tests::World, service::GatewayService, worker::Work};

    #[tokio::test]
    async fn api_create_get_delete_project() -> anyhow::Result<()> {
        let world = World::new().await?;
        let service = Arc::new(GatewayService::init(world.context().args.clone()).await);

        let (sender, receiver) = channel::<Work>(256);
        service.set_sender(Some(sender));

        let mut router = make_api(Arc::clone(&service));

        let req = Request::builder()
            .method("GET")
            .uri("/users/neo")
            .body(Body::empty())
            .unwrap();
        let mut resp = router.call(req).await?;
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

        let user = service.create_user("neo".parse().unwrap()).await?;

        let req = Request::builder()
            .method("GET")
            .uri("/users/neo")
            .body(Body::empty())
            .unwrap();
        let resp = router.call(req).await?;
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

        let mut values = vec![];
        let header = Authorization::basic("", user.key.as_str());
        header.encode(&mut values);
        let value = values.pop().unwrap();
        let req = Request::builder()
            .method("GET")
            .uri("/users/neo")
            .header(Authorization::<Basic>::name(), value)
            .body(Body::empty())
            .unwrap();
        let resp = router.call(req).await?;
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);

        service.set_super_user(&user.name, true).await?;

        let mut values = vec![];
        let header = Authorization::basic("", user.key.as_str());
        header.encode(&mut values);
        let value = values.pop().unwrap();
        let req = Request::builder()
            .method("GET")
            .uri("/users/neo")
            .header(Authorization::<Basic>::name(), value)
            .body(Body::empty())
            .unwrap();
        let resp = router.call(req).await?;
        assert_eq!(resp.status(), StatusCode::OK);

        Ok(())
    }
}
