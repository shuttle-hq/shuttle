use std::sync::Arc;
use std::time::Duration;

use axum::body::{Body, BoxBody};
use axum::extract::{Extension, Path};
use axum::http::Request;
use axum::response::Response;
use axum::routing::{any, get, post};
use axum::{Json as AxumJson, Router};
use http::StatusCode;
use instant_acme::{
    Account, AccountCredentials, ChallengeType, Identifier, NewAccount, NewOrder, OrderStatus,
};
use rcgen::{Certificate, CertificateParams, DistinguishedName};
use serde::{Deserialize, Serialize};
use shuttle_common::models::error::ErrorKind;
use shuttle_common::models::{project, user};
use tokio::sync::mpsc::Sender;
use tokio::time::sleep;
use tower_http::trace::TraceLayer;
use tracing::{debug, debug_span, error, field, trace, Span};

use crate::auth::{Admin, ScopedUser, User};
use crate::task::{self, BoxedTask};
use crate::worker::WORKER_QUEUE_SIZE;
use crate::{AccountName, Error, Fqdn, GatewayService, ProjectName};

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

async fn get_user(
    Extension(service): Extension<Arc<GatewayService>>,
    Path(account_name): Path<AccountName>,
    _: Admin,
) -> Result<AxumJson<user::Response>, Error> {
    let user = User::retrieve_from_account_name(&service, account_name).await?;

    Ok(AxumJson(user.into()))
}

async fn post_user(
    Extension(service): Extension<Arc<GatewayService>>,
    Path(account_name): Path<AccountName>,
    _: Admin,
) -> Result<AxumJson<user::Response>, Error> {
    let user = service.create_user(account_name).await?;

    Ok(AxumJson(user.into()))
}

async fn get_project(
    Extension(service): Extension<Arc<GatewayService>>,
    ScopedUser { scope, .. }: ScopedUser,
) -> Result<AxumJson<project::Response>, Error> {
    let state = service.find_project(&scope).await?.into();
    let response = project::Response {
        name: scope.to_string(),
        state,
    };

    Ok(AxumJson(response))
}

async fn post_project(
    Extension(service): Extension<Arc<GatewayService>>,
    Extension(sender): Extension<Sender<BoxedTask>>,
    User { name, .. }: User,
    Path(project): Path<ProjectName>,
) -> Result<AxumJson<project::Response>, Error> {
    let state = service
        .create_project(project.clone(), name.clone())
        .await?;

    service
        .new_task()
        .project(project.clone())
        .account(name.clone())
        .send(&sender)
        .await?;

    let response = project::Response {
        name: project.to_string(),
        state: state.into(),
    };

    Ok(AxumJson(response))
}

async fn delete_project(
    Extension(service): Extension<Arc<GatewayService>>,
    Extension(sender): Extension<Sender<BoxedTask>>,
    ScopedUser {
        scope: _,
        user: User { name, .. },
    }: ScopedUser,
    Path(project): Path<ProjectName>,
) -> Result<AxumJson<project::Response>, Error> {
    let project_name = project.clone();

    let state = service.find_project(&project_name).await?;

    let mut response = project::Response {
        name: project_name.to_string(),
        state: state.into(),
    };

    if response.state == shuttle_common::models::project::State::Destroyed {
        return Ok(AxumJson(response));
    }

    // if project exists and isn't `Destroyed`, send destroy task
    service
        .new_task()
        .project(project)
        .account(name)
        .and_then(task::destroy())
        .send(&sender)
        .await?;

    response.state = shuttle_common::models::project::State::Destroying;

    Ok(AxumJson(response))
}

async fn route_project(
    Extension(service): Extension<Arc<GatewayService>>,
    ScopedUser { scope, .. }: ScopedUser,
    req: Request<Body>,
) -> Result<Response<Body>, Error> {
    service.route(&scope, req).await
}

async fn get_status(Extension(sender): Extension<Sender<BoxedTask>>) -> Response<Body> {
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

async fn revive_projects(
    _: Admin,
    Extension(service): Extension<Arc<GatewayService>>,
    Extension(sender): Extension<Sender<BoxedTask>>,
) -> Result<(), Error> {
    crate::project::exec::revive(service, sender)
        .await
        .map_err(|_| Error::from_kind(ErrorKind::Internal))
}

async fn create_acme_account(
    _: Admin,
    Path(email): Path<String>,
) -> Result<AxumJson<serde_json::Value>, Error> {
    trace!(email, "creating acme account");

    let account = NewAccount {
        contact: &[&format!("mailto:{email}")],
        terms_of_service_agreed: true,
        only_return_existing: false,
    };
    // TODO: change to production
    let account = Account::create(&account, LetsEncrypt::Staging.url())
        .await
        .map_err(|error| {
            error!(%error, "got error while creating acme account");
            Error::from_kind(ErrorKind::Internal)
        })?;

    let credentials = serde_json::to_value(account.credentials()).map_err(|error| {
        error!(%error, "got error while extracting credentials from acme account");
        Error::from_kind(ErrorKind::Internal)
    })?;

    Ok(AxumJson(credentials))
}

async fn request_acme_certificate(
    Extension(service): Extension<Arc<GatewayService>>,
    _: Admin,
    Path(fqdn): Path<Fqdn>,
    AxumJson(credentials): AxumJson<AccountCredentials<'_>>,
) -> Result<AxumJson<serde_json::Value>, Error> {
    trace!(%fqdn, "requesting acme certificate");

    let account = Account::from_credentials(credentials).map_err(|error| {
        error!(
            error = &error as &dyn std::error::Error,
            "failed to convert acme credentials into account"
        );
        Error::from_kind(ErrorKind::Internal)
    })?;

    let (mut order, state) = account
        .new_order(&NewOrder {
            identifiers: &[Identifier::Dns(fqdn.to_string())],
        })
        .await
        .map_err(|error| {
            error!(%error, "failed to order certificate");
            Error::from(ErrorKind::Internal)
        })?;

    let authorizations = order
        .authorizations(&state.authorizations)
        .await
        .map_err(|error| {
            error!(%error, "failed to get authorizations information");
            Error::from(ErrorKind::Internal)
        })?;

    // There should only ever be 1 authorization as we only provide 1 domain at a time
    debug_assert!(authorizations.len() == 1);
    let authorization = &authorizations[0];

    let challenge = authorization
        .challenges
        .iter()
        .find(|c| c.r#type == ChallengeType::Http01)
        .ok_or_else(|| {
            error!("http-01 challenge not found");
            Error::from(ErrorKind::Internal)
        })?;

    service
        .add_http01_challenge_authorization(
            challenge.token.clone(),
            order.key_authorization(challenge),
        )
        .await;

    order
        .set_challenge_ready(&challenge.url)
        .await
        .map_err(|error| {
            error!(%error, "failed to mark challenge as ready");
            Error::from(ErrorKind::Internal)
        })?;

    // Exponential backoff until order changes status
    let mut tries = 1u8;
    let mut delay = Duration::from_millis(250);
    let state = loop {
        sleep(delay).await;
        let state = order.state().await.map_err(|error| {
            error!(%error, "got error while fetching state");
            Error::from(ErrorKind::Internal)
        })?;

        trace!(?state, "order state refreshed");
        match state.status {
            OrderStatus::Ready => break state,
            OrderStatus::Invalid => {
                service
                    .remove_http01_challenge_authorization(&challenge.token)
                    .await;
                return Err(Error::from(ErrorKind::Internal));
            }
            OrderStatus::Pending => {
                delay *= 2;
                tries += 1;
                if tries < 5 {
                    trace!(?state, tries, attempt_in=?delay, "order not yet ready");
                } else {
                    error!(?state, tries, "order not ready in 5 tries");
                    service
                        .remove_http01_challenge_authorization(&challenge.token)
                        .await;
                    return Err(Error::from(ErrorKind::Internal));
                }
            }
            _ => unreachable!(),
        }
    };

    service
        .remove_http01_challenge_authorization(&challenge.token)
        .await;

    let certificate = {
        let mut params = CertificateParams::new(vec![fqdn.to_string()]);
        params.distinguished_name = DistinguishedName::new();
        Certificate::from_params(params).map_err(|error| {
            error!(%error, "failed to create certificate");
            Error::from(ErrorKind::Internal)
        })?
    };
    let signing_request = certificate.serialize_request_der().map_err(|error| {
        error!(%error, "failed to create certificate signing request");
        Error::from(ErrorKind::Internal)
    })?;

    let certificate_chain = order
        .finalize(&signing_request, &state.finalize)
        .await
        .map_err(|error| {
            error!(%error, "failed to finalize certificate request");
            Error::from(ErrorKind::Internal)
        })?;

    // TODO: save certificate to database

    Ok(AxumJson(serde_json::json!({
        "certificate": certificate_chain,
        "private_key": certificate.serialize_private_key_pem(),
        "account_credentials": account.credentials(),
    })))
}

pub fn make_api(service: Arc<GatewayService>, sender: Sender<BoxedTask>) -> Router<Body> {
    debug!("making api route");

    Router::<Body>::new()
        .route(
            "/",
            get(get_status)
        )
        .route(
            "/projects/:project",
            get(get_project).delete(delete_project).post(post_project)
        )
        .route("/users/:account_name", get(get_user).post(post_user))
        .route("/projects/:project/*any", any(route_project))
        .route("/admin/revive", post(revive_projects))
        .route("/admin/acme/:email", post(create_acme_account))
        .route("/admin/acme/request/:fqdn", post(request_acme_certificate))
        .layer(Extension(service))
        .layer(Extension(sender))
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(|request: &Request<Body>| {
                    debug_span!("request", http.uri = %request.uri(), http.method = %request.method(), http.status_code = field::Empty, account.name = field::Empty, account.project = field::Empty)
                })
                .on_response(
                    |response: &Response<BoxBody>, latency: Duration, span: &Span| {
                        span.record("http.status_code", response.status().as_u16());
                        debug!(latency = format_args!("{} ns", latency.as_nanos()), "finished processing request");
                    },
                ),
        )
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
        let service = Arc::new(GatewayService::init(world.args(), world.pool()).await);

        let (sender, mut receiver) = channel::<BoxedTask>(256);
        tokio::spawn(async move {
            while receiver.recv().await.is_some() {
                // do not do any work with inbound requests
            }
        });

        let mut router = make_api(Arc::clone(&service), sender);

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

        let authorization = Authorization::bearer(neo.key.as_str()).unwrap();

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

        let authorization = Authorization::bearer(trinity.key.as_str()).unwrap();

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

        // delete returns 404 for project that doesn't exist
        router
            .call(delete_project("resurrections").with_header(&authorization))
            .map_ok(|resp| {
                assert_eq!(resp.status(), StatusCode::NOT_FOUND);
            })
            .await
            .unwrap();

        Ok(())
    }

    #[tokio::test]
    async fn api_create_get_users() -> anyhow::Result<()> {
        let world = World::new().await;
        let service = Arc::new(GatewayService::init(world.args(), world.pool()).await);

        let (sender, mut receiver) = channel::<BoxedTask>(256);
        tokio::spawn(async move {
            while receiver.recv().await.is_some() {
                // do not do any work with inbound requests
            }
        });

        let mut router = make_api(Arc::clone(&service), sender);

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

        let authorization = Authorization::bearer(user.key.as_str()).unwrap();

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

    #[tokio::test(flavor = "multi_thread")]
    async fn status() {
        let world = World::new().await;
        let service = Arc::new(GatewayService::init(world.args(), world.pool()).await);

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

        let mut router = make_api(Arc::clone(&service), sender.clone());

        let get_status = || {
            Request::builder()
                .method("GET")
                .uri("/")
                .body(Body::empty())
                .unwrap()
        };

        let resp = router.call(get_status()).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let neo: AccountName = "neo".parse().unwrap();
        let matrix: ProjectName = "matrix".parse().unwrap();

        let neo = service.create_user(neo).await.unwrap();
        let authorization = Authorization::bearer(neo.key.as_str()).unwrap();

        let create_project = Request::builder()
            .method("POST")
            .uri(format!("/projects/{matrix}"))
            .body(Body::empty())
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
