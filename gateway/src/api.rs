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
    service.create_user(account_name).await.map(AxumJson)
}

async fn get_project(
    Extension(service): Extension<Arc<GatewayService>>,
    ScopedUser { scope, .. }: ScopedUser,
) -> Result<AxumJson<Project>, Error> {
    service.find_project(&scope).await.map(AxumJson)
}

async fn post_project(
    Extension(service): Extension<Arc<GatewayService>>,
    User { name, .. }: User,
    Path(project): Path<ProjectName>,
) -> Result<AxumJson<Project>, Error> {
    service.create_project(project, name).await.map(AxumJson)
}

async fn delete_project(
    Extension(service): Extension<Arc<GatewayService>>,
    ScopedUser { scope, user: User { name, .. } }: ScopedUser,
    Path(project): Path<ProjectName>,
) -> Result<(), Error> {
    service.destroy_project(project, name).await
}

async fn route_project(
    Extension(service): Extension<Arc<GatewayService>>,
    ScopedUser { scope, .. }: ScopedUser,
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

    use futures::TryFutureExt;
    use tokio::sync::mpsc::channel;

    use axum::{
        body::{Body, HttpBody},
        headers::{
            authorization::{self, Basic},
            Authorization, Header,
        },
        http::Request,
    };
    use tower::Service;

    use super::*;

    use crate::{service::GatewayService, tests::World, worker::Work};

    mod request_builder_ext {
        pub trait Sealed {}

        impl Sealed for axum::http::request::Builder {}

        impl<'r> Sealed for &'r mut axum::headers::HeaderMap {}

        impl<B> Sealed for axum::http::Request<B> {}
    }

    pub trait RequestBuilderExt: Sized + request_builder_ext::Sealed {
        fn with_header<H: axum::headers::Header>(self, header: &H) -> Self;
    }

    impl RequestBuilderExt for axum::http::request::Builder {
        fn with_header<H: Header>(mut self, header: &H) -> Self {
            self.headers_mut().unwrap().with_header(header);
            self
        }
    }

    impl<'r> RequestBuilderExt for &'r mut axum::headers::HeaderMap {
        fn with_header<H: axum::headers::Header>(self, header: &H) -> Self {
            let mut buf = vec![];
            header.encode(&mut buf);
            self.append(H::name(), buf.pop().unwrap());
            self
        }
    }

    impl<B> RequestBuilderExt for Request<B> {
        fn with_header<H: axum::headers::Header>(mut self, header: &H) -> Self {
            self.headers_mut().with_header(header);
            self
        }
    }

    #[tokio::test]
    async fn api_create_get_delete_projects() -> anyhow::Result<()> {
        let world = World::new().await?;
        let service = Arc::new(GatewayService::init(world.context().args.clone()).await);

        let (sender, mut receiver) = channel::<Work>(256);
        tokio::spawn(async move {
            while let Some(_) = receiver.recv().await {
                // do not do any work with inbound requests
            }
        });
        service.set_sender(Some(sender)).await.unwrap();

        let mut router = make_api(Arc::clone(&service));

        let neo = service.create_user("neo".parse().unwrap()).await?;

        let create_project = |project: &str| {
            Request::builder()
                .method("POST")
                .uri(format!("/projects/{project}"))
                .body(Body::empty())
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

        let authorization = Authorization::basic("", neo.key.as_str());

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

        let trinity = service.create_user("trinity".parse().unwrap()).await?;

        let authorization = Authorization::basic("", trinity.key.as_str());

        router
            .call(get_project("reloaded").with_header(&authorization))
            .map_ok(|resp| assert_eq!(resp.status(), StatusCode::FORBIDDEN))
            .await
            .unwrap();

        router
            .call(delete_project("reloaded").with_header(&authorization))
            .map_ok(|resp| {
                assert_eq!(resp.status(), StatusCode::FORBIDDEN);
            })
            .await
            .unwrap();

        service
            .set_super_user(&"trinity".parse().unwrap(), true)
            .await?;

        router
            .call(get_project("reloaded").with_header(&authorization))
            .map_ok(|resp| assert_eq!(resp.status(), StatusCode::OK))
            .await
            .unwrap();

        router
            .call(delete_project("reloaded").with_header(&authorization))
            .map_ok(|resp| {
                assert_eq!(resp.status(), StatusCode::OK);
            })
            .await
            .unwrap();

        Ok(())
    }

    #[tokio::test]
    async fn api_create_get_users() -> anyhow::Result<()> {
        let world = World::new().await?;
        let service = Arc::new(GatewayService::init(world.context().args.clone()).await);

        let mut router = make_api(Arc::clone(&service));

        let get_neo = || {
            Request::builder()
                .method("GET")
                .uri("/users/neo")
                .body(Body::empty())
                .unwrap()
        };

        let post_trinity = || {
            Request::builder()
                .method("POST")
                .uri("/users/trinity")
                .body(Body::empty())
                .unwrap()
        };

        router
            .call(get_neo())
            .map_ok(|resp| {
                assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
            })
            .await
            .unwrap();

        let user = service.create_user("neo".parse().unwrap()).await?;

        router
            .call(get_neo())
            .map_ok(|resp| {
                assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
            })
            .await
            .unwrap();

        let authorization = Authorization::basic("", user.key.as_str());

        router
            .call(get_neo().with_header(&authorization))
            .map_ok(|resp| {
                assert_eq!(resp.status(), StatusCode::FORBIDDEN);
            })
            .await
            .unwrap();

        router
            .call(post_trinity().with_header(&authorization))
            .map_ok(|resp| assert_eq!(resp.status(), StatusCode::FORBIDDEN))
            .await
            .unwrap();

        service.set_super_user(&user.name, true).await?;

        router
            .call(get_neo().with_header(&authorization))
            .map_ok(|resp| {
                assert_eq!(resp.status(), StatusCode::OK);
            })
            .await
            .unwrap();

        router
            .call(post_trinity().with_header(&authorization))
            .map_ok(|resp| assert_eq!(resp.status(), StatusCode::OK))
            .await
            .unwrap();

        router
            .call(post_trinity().with_header(&authorization))
            .map_ok(|resp| assert_eq!(resp.status(), StatusCode::BAD_REQUEST))
            .await
            .unwrap();

        Ok(())
    }
}
