#[macro_use]
extern crate async_trait;

use std::convert::Infallible;
use std::error::Error as StdError;
use std::fmt::Formatter;
use std::io;
use std::pin::Pin;
use std::str::FromStr;

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use bollard::Docker;
use convert_case::{Case, Casing};
use futures::prelude::*;
use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::json;
use tracing::error;

pub mod api;
pub mod args;
pub mod auth;
pub mod project;
pub mod proxy;
pub mod service;
pub mod worker;

use crate::service::{ContainerSettings, GatewayService};

static PROJECT_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new("^[a-zA-Z0-9\\-_]{3,64}$").unwrap());

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    KeyMissing,
    BadHost,
    KeyMalformed,
    Unauthorized,
    Forbidden,
    UserNotFound,
    UserAlreadyExists,
    ProjectNotFound,
    InvalidProjectName,
    ProjectAlreadyExists,
    ProjectNotReady,
    ProjectUnavailable,
    InvalidOperation,
    Internal,
    NotReady,
}

impl std::fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let formatted = format!("{:?}", self).to_case(Case::Snake);
        write!(f, "{}", formatted)
    }
}

/// Server-side errors that do not have to do with the user runtime
/// should be [`Error`]s.
///
/// All [`Error`] have an [`ErrorKind`] and an (optional) source.

/// [`Error] is safe to be used as error variants to axum endpoints
/// return types as their [`IntoResponse`] implementation does not
/// leak any sensitive information.
#[derive(Debug)]
pub struct Error {
    kind: ErrorKind,
    source: Option<Box<dyn StdError + Sync + Send + 'static>>,
}

impl Error {
    pub fn source<E: StdError + Sync + Send + 'static>(kind: ErrorKind, err: E) -> Self {
        Self {
            kind,
            source: Some(Box::new(err)),
        }
    }

    pub fn custom<S: AsRef<str>>(kind: ErrorKind, message: S) -> Self {
        Self {
            kind,
            source: Some(Box::new(io::Error::new(
                io::ErrorKind::Other,
                message.as_ref().to_string(),
            ))),
        }
    }

    pub fn from_kind(kind: ErrorKind) -> Self {
        Self { kind, source: None }
    }

    pub fn kind(&self) -> ErrorKind {
        self.kind
    }
}

impl From<ErrorKind> for Error {
    fn from(kind: ErrorKind) -> Self {
        Self::from_kind(kind)
    }
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        error!(error = %self, "request had an error");

        let (status, error_message) = match self.kind {
            ErrorKind::Internal => (StatusCode::INTERNAL_SERVER_ERROR, "internal server error"),
            ErrorKind::KeyMissing => (StatusCode::UNAUTHORIZED, "request is missing a key"),
            ErrorKind::KeyMalformed => (StatusCode::BAD_REQUEST, "request has an invalid key"),
            ErrorKind::BadHost => (StatusCode::BAD_REQUEST, "the 'Host' header is invalid"),
            ErrorKind::UserNotFound => (StatusCode::NOT_FOUND, "user not found"),
            ErrorKind::UserAlreadyExists => (StatusCode::BAD_REQUEST, "user already exists"),
            ErrorKind::ProjectNotFound => (StatusCode::NOT_FOUND, "project not found"),
            ErrorKind::ProjectNotReady => (StatusCode::SERVICE_UNAVAILABLE, "project not ready"),
            ErrorKind::ProjectUnavailable => {
                (StatusCode::BAD_GATEWAY, "project returned invalid response")
            }
            ErrorKind::InvalidProjectName => (StatusCode::BAD_REQUEST, "invalid project name"),
            ErrorKind::InvalidOperation => (
                StatusCode::BAD_REQUEST,
                "the requested operation is invalid",
            ),
            ErrorKind::ProjectAlreadyExists => (
                StatusCode::BAD_REQUEST,
                "a project with the same name already exists",
            ),
            ErrorKind::Unauthorized => (StatusCode::UNAUTHORIZED, "unauthorized"),
            ErrorKind::Forbidden => (StatusCode::FORBIDDEN, "forbidden"),
            ErrorKind::NotReady => (StatusCode::INTERNAL_SERVER_ERROR, "not ready yet"),
        };
        (status, Json(json!({ "error": error_message }))).into_response()
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.kind)?;
        if let Some(source) = self.source.as_ref() {
            write!(f, ": ")?;
            source.fmt(f)?;
        }
        Ok(())
    }
}

impl StdError for Error {}

#[derive(Debug, sqlx::Type, Serialize, Clone, PartialEq, Eq)]
#[sqlx(transparent)]
pub struct ProjectName(String);

impl<'de> Deserialize<'de> for ProjectName {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        String::deserialize(deserializer)?
            .parse()
            .map_err(|err| <D::Error as serde::de::Error>::custom(err))
    }
}

impl FromStr for ProjectName {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if PROJECT_REGEX.is_match(s) {
            Ok(Self(s.to_string()))
        } else {
            Err(Error::from_kind(ErrorKind::InvalidProjectName))
        }
    }
}

impl std::fmt::Display for ProjectName {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, sqlx::Type, Serialize)]
#[sqlx(transparent)]
pub struct AccountName(String);

impl FromStr for AccountName {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(s.to_string()))
    }
}

impl std::fmt::Display for AccountName {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl<'de> Deserialize<'de> for AccountName {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        String::deserialize(deserializer)?
            .parse()
            .map_err(|_err| todo!())
    }
}

pub trait Context<'c>: Send + Sync {
    fn docker(&self) -> &'c Docker;

    fn container_settings(&self) -> &'c ContainerSettings;
}

#[async_trait]
pub trait Service<'c> {
    type Context: Context<'c>;

    type State: EndState<'c>;

    type Error;

    /// Asks for the latest available context for task execution
    fn context(&'c self) -> Self::Context;

    /// Commit a state update to persistence
    async fn update(&mut self, state: &Self::State) -> Result<(), Self::Error>;
}

/// A generic state which can, when provided with a [`Context`], do
/// some work and advance itself
#[async_trait]
pub trait State<'c>: Send + Sized + Clone {
    type Next;

    type Error;

    async fn next<C: Context<'c>>(self, ctx: &C) -> Result<Self::Next, Self::Error>;
}

/// A [`State`] which contains all its transitions, including
/// failures
pub trait EndState<'c>
where
    Self: State<'c, Error = Infallible, Next = Self>,
{
    type ErrorVariant;

    fn is_done(&self) -> bool;

    fn into_result(self) -> Result<Self, Self::ErrorVariant>;
}

pub type StateTryStream<'c, St, Err> = Pin<Box<dyn Stream<Item = Result<St, Err>> + Send + 'c>>;

pub trait EndStateExt<'c>: EndState<'c> {
    /// Convert the state into a [`TryStream`] that yields
    /// the generated states.
    ///
    /// This stream will not end.
    fn into_stream<Ctx>(self, ctx: Ctx) -> StateTryStream<'c, Self, Self::ErrorVariant>
    where
        Self: 'c,
        Ctx: 'c + Context<'c>,
    {
        Box::pin(stream::try_unfold((self, ctx), |(state, ctx)| async move {
            state
                .next(&ctx)
                .await
                .unwrap() // EndState's `next` is Infallible
                .into_result()
                .map(|state| Some((state.clone(), (state, ctx))))
        }))
    }
}

impl<'c, S> EndStateExt<'c> for S where S: EndState<'c> {}

pub trait IntoEndState<'c, E>
where
    E: EndState<'c>,
{
    fn into_end_state(self) -> Result<E, Infallible>;
}

impl<'c, E, S, Err> IntoEndState<'c, E> for Result<S, Err>
where
    E: EndState<'c> + From<S> + From<Err>,
{
    fn into_end_state(self) -> Result<E, Infallible> {
        self.map(|s| E::from(s)).or_else(|err| Ok(E::from(err)))
    }
}

#[async_trait]
pub trait Refresh: Sized {
    type Error: StdError;

    async fn refresh<'c, C: Context<'c>>(self, ctx: &C) -> Result<Self, Self::Error>;
}

#[cfg(test)]
pub mod tests {
    use std::env;
    use std::io::Read;
    use std::net::SocketAddr;
    use std::sync::Arc;
    use std::time::Duration;

    use anyhow::{anyhow, Context as AnyhowContext};
    use axum::headers::Authorization;
    use bollard::Docker;
    use futures::prelude::*;
    use hyper::client::HttpConnector;
    use hyper::http::uri::Scheme;
    use hyper::http::Uri;
    use hyper::{Body, Client as HyperClient, Request, Response, StatusCode};
    use rand::distributions::{Alphanumeric, DistString, Distribution, Uniform};
    use shuttle_common::service;
    use sqlx::SqlitePool;
    use tokio::sync::mpsc::channel;
    use tracing::info;

    use crate::api::make_api;
    use crate::args::StartArgs;
    use crate::auth::User;
    use crate::project::Project;
    use crate::proxy::make_proxy;
    use crate::service::{ContainerSettings, GatewayService, MIGRATIONS};
    use crate::worker::Worker;
    use crate::{Context, EndState};

    macro_rules! value_block_helper {
        ($next:ident, $block:block) => {
            $block
        };
        ($next:ident,) => {
            $next
        };
    }

    macro_rules! assert_stream_matches {
        (
            $stream:ident,
            $(#[assertion = $assert:literal])?
                $($pattern:pat_param)|+ $(if $guard:expr)? $(=> $more:block)?,
        ) => {{
            let next = ::futures::stream::StreamExt::next(&mut $stream)
                .await
                .expect("Stream ended before the last of assertions");

            match &next {
                $($pattern)|+ $(if $guard)? => {
                    print!("{}", ::colored::Colorize::green(::colored::Colorize::bold("[ok]")));
                    $(print!(" {}", $assert);)?
                        print!("\n");
                    crate::tests::value_block_helper!(next, $($more)?)
                },
                _ => {
                    eprintln!("{} {:#?}", ::colored::Colorize::red(::colored::Colorize::bold("[err]")), next);
                    eprint!("{}", ::colored::Colorize::red(::colored::Colorize::bold("Assertion failed")));
                    $(eprint!(": {}", $assert);)?
                        eprint!("\n");
                    panic!("State mismatch")
                }
            }
        }};
        (
            $stream:ident,
            $(#[$($meta:tt)*])*
                $($pattern:pat_param)|+ $(if $guard:expr)? $(=> $more:block)?,
            $($(#[$($metas:tt)*])* $($patterns:pat_param)|+ $(if $guards:expr)? $(=> $mores:block)?,)+
        ) => {{
            assert_stream_matches!(
                $stream,
                $(#[$($meta)*])* $($pattern)|+ $(if $guard)? => {
                    $($more)?
                        assert_stream_matches!(
                            $stream,
                            $($(#[$($metas)*])* $($patterns)|+ $(if $guards)? $(=> $mores)?,)+
                        )
                },
            )
        }};
    }

    macro_rules! assert_matches {
        {
            $ctx:ident,
            $state:expr,
            $($(#[$($meta:tt)*])* $($patterns:pat_param)|+ $(if $guards:expr)? $(=> $mores:block)?,)+
        } => {{
            let state = $state;
            let mut stream = crate::EndStateExt::into_stream(state, $ctx);
            assert_stream_matches!(
                stream,
                $($(#[$($meta)*])* $($patterns)|+ $(if $guards)? $(=> $mores)?,)+
            )
        }}
    }

    macro_rules! assert_err_kind {
        {
            $left:expr, ErrorKind::$right:ident
        } => {{
            let left: Result<_, crate::Error> = $left;
            assert_eq!(
                left.map_err(|err| err.kind()),
                Err(crate::ErrorKind::$right)
            );
        }};
    }

    macro_rules! timed_loop {
        (wait: $wait:literal$(, max: $max:literal)?, $block:block) => {{
            #[allow(unused_mut)]
            #[allow(unused_variables)]
            let mut tries = 0;
            loop {
                $block
                    tries += 1;
                $(if tries > $max {
                    panic!("timed out in the loop");
                })?
                    ::tokio::time::sleep(::std::time::Duration::from_secs($wait)).await;
            }
        }};
    }

    pub(crate) use {assert_err_kind, assert_matches, assert_stream_matches, value_block_helper};

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
        fn with_header<H: axum::headers::Header>(mut self, header: &H) -> Self {
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

    pub struct Client<C = HttpConnector, B = Body> {
        target: SocketAddr,
        hyper: Option<HyperClient<C, B>>,
    }

    impl<C, B> Client<C, B> {
        pub fn new<A: Into<SocketAddr>>(target: A) -> Self {
            Self {
                target: target.into(),
                hyper: None,
            }
        }

        pub fn with_hyper_client(mut self, client: HyperClient<C, B>) -> Self {
            self.hyper = Some(client);
            self
        }
    }

    impl Client<HttpConnector, Body> {
        pub async fn request(
            &self,
            mut req: Request<Body>,
        ) -> Result<Response<Vec<u8>>, hyper::Error> {
            if req.uri().authority().is_none() {
                let mut uri = req.uri().clone().into_parts();
                uri.scheme = Some(Scheme::HTTP);
                uri.authority = Some(self.target.to_string().parse().unwrap());
                *req.uri_mut() = Uri::from_parts(uri).unwrap();
            }
            self.hyper
                .as_ref()
                .unwrap()
                .request(req)
                .and_then(|mut resp| async move {
                    let body = resp
                        .body_mut()
                        .try_fold(Vec::new(), |mut acc, x| async move {
                            acc.extend(x);
                            Ok(acc)
                        })
                        .await?;
                    let (parts, _) = resp.into_parts();
                    Ok(Response::from_parts(parts, body))
                })
                .await
        }
    }

    pub struct World {
        docker: Docker,
        settings: ContainerSettings,
        args: StartArgs,
        hyper: HyperClient<HttpConnector, Body>,
        pool: SqlitePool,
    }

    #[derive(Clone, Copy)]
    pub struct WorldContext<'c> {
        pub docker: &'c Docker,
        pub container_settings: &'c ContainerSettings,
        pub hyper: &'c HyperClient<HttpConnector, Body>,
    }

    impl World {
        pub async fn new() -> Self {
            let docker = Docker::connect_with_local_defaults().unwrap();

            docker
                .list_images::<&str>(None)
                .await
                .context(anyhow!("A docker daemon does not seem accessible",))
                .unwrap();

            let control: i16 = Uniform::from(9000..10000).sample(&mut rand::thread_rng());
            let user = control + 1;
            let control = format!("127.0.0.1:{control}").parse().unwrap();
            let user = format!("127.0.0.1:{user}").parse().unwrap();

            let prefix = format!(
                "shuttle_test_{}_",
                Alphanumeric.sample_string(&mut rand::thread_rng(), 4)
            );

            let image = env::var("SHUTTLE_TESTS_RUNTIME_IMAGE")
                .unwrap_or("public.ecr.aws/shuttle/backend:latest".to_string());

            let network_name =
                env::var("SHUTTLE_TESTS_NETWORK").unwrap_or("shuttle_default".to_string());

            let provisioner_host = "provisioner".to_string();

            let args = StartArgs {
                control,
                user,
                image,
                prefix,
                provisioner_host,
                network_name,
            };

            let settings = ContainerSettings::builder(&docker).from_args(&args).await;

            let hyper = HyperClient::builder().build(HttpConnector::new());

            let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
            MIGRATIONS.run(&pool).await.unwrap();

            Self {
                docker,
                settings,
                args,
                hyper,
                pool,
            }
        }

        pub fn args(&self) -> StartArgs {
            self.args.clone()
        }

        pub fn pool(&self) -> SqlitePool {
            self.pool.clone()
        }

        pub fn client<A: Into<SocketAddr>>(&self, addr: A) -> Client {
            Client::new(addr).with_hyper_client(self.hyper.clone())
        }
    }

    impl World {
        pub fn context<'c>(&'c self) -> WorldContext<'c> {
            WorldContext {
                docker: &self.docker,
                container_settings: &self.settings,
                hyper: &self.hyper,
            }
        }
    }

    impl<'c> Context<'c> for WorldContext<'c> {
        fn docker(&self) -> &'c Docker {
            self.docker
        }

        fn container_settings(&self) -> &'c ContainerSettings {
            self.container_settings
        }
    }

    #[tokio::test]
    async fn end_to_end() {
        let world = World::new().await;
        let service = Arc::new(GatewayService::init(world.args(), world.pool()).await);
        let worker = Worker::new(Arc::clone(&service));

        let (log_out, mut log_in) = channel(256);
        tokio::spawn({
            let sender = worker.sender();
            async move {
                while let Some(work) = log_in.recv().await {
                    info!("work: {work:?}");
                    sender.send(work).await.unwrap()
                }
                info!("work channel closed");
            }
        });
        service.set_sender(Some(log_out)).await.unwrap();

        let base_port = loop {
            let port = portpicker::pick_unused_port().unwrap();
            if portpicker::is_free_tcp(port + 1) {
                break port;
            }
        };

        let api = make_api(Arc::clone(&service));
        let api_addr = format!("127.0.0.1:{}", base_port).parse().unwrap();
        let serve_api = hyper::Server::bind(&api_addr).serve(api.into_make_service());
        let api_client = world.client(api_addr.clone());

        let proxy = make_proxy(Arc::clone(&service));
        let proxy_addr = format!("127.0.0.1:{}", base_port + 1).parse().unwrap();
        let serve_proxy = hyper::Server::bind(&proxy_addr).serve(proxy);
        let proxy_client = world.client(proxy_addr.clone());

        let _gateway = tokio::spawn(async move {
            tokio::select! {
                _ = worker.start() => {},
                _ = serve_api => {},
                _ = serve_proxy => {}
            }
        });

        let User { key, name, .. } = service.create_user("neo".parse().unwrap()).await.unwrap();
        service.set_super_user(&name, true).await.unwrap();

        let User { key, .. } = api_client
            .request(
                Request::post("/users/trinity")
                    .with_header(&Authorization::bearer(key.as_str()).unwrap())
                    .body(Body::empty())
                    .unwrap(),
            )
            .map_ok(|resp| {
                assert_eq!(resp.status(), StatusCode::OK);
                serde_json::from_slice(resp.body()).unwrap()
            })
            .await
            .unwrap();

        let authorization = Authorization::bearer(key.as_str()).unwrap();

        api_client
            .request(
                Request::post("/projects/matrix")
                    .with_header(&authorization)
                    .body(Body::empty())
                    .unwrap(),
            )
            .map_ok(|resp| {
                assert_eq!(resp.status(), StatusCode::OK);
            })
            .await
            .unwrap();

        let _ = timed_loop!(wait: 1, max: 12, {
            let project: Project = api_client
                .request(
                    Request::get("/projects/matrix")
                        .with_header(&authorization)
                        .body(Body::empty())
                        .unwrap(),
                )
                .map_ok(|resp| {
                    assert_eq!(resp.status(), StatusCode::OK);
                    serde_json::from_slice(resp.body()).unwrap()
                })
                .await
                .unwrap();

            // Equivalent to `::Ready(_)`
            if let Some(target_ip) = project.target_addr().unwrap() {
                break target_ip;
            } else if project.is_done() {
                panic!("project finished without providing an IP: {:#?}", project);
            }

            tokio::time::sleep(Duration::from_secs(1)).await;
        });

        api_client
            .request(
                Request::get("/projects/matrix/status")
                    .with_header(&authorization)
                    .body(Body::empty())
                    .unwrap(),
            )
            .map_ok(|resp| assert_eq!(resp.status(), StatusCode::OK))
            .await
            .unwrap();

        // === deployment test BEGIN ===
        api_client
            .request({
                let mut data = Vec::new();
                let mut f = std::fs::File::open("tests/hello_world.crate").unwrap();
                f.read_to_end(&mut data).unwrap();
                Request::post("/projects/matrix/projects/matrix")
                    .with_header(&authorization)
                    .body(Body::from(data))
                    .unwrap()
            })
            .map_ok(|resp| assert_eq!(resp.status(), StatusCode::OK))
            .await
            .unwrap();

        timed_loop!(wait: 1, max: 600, {
            let service: service::Summary = api_client
                .request(
                    Request::get("/projects/matrix/projects/matrix/summary")
                        .with_header(&authorization)
                        .body(Body::empty())
                        .unwrap(),
                )
                .map_ok(|resp| {
                    assert_eq!(resp.status(), StatusCode::OK);
                    serde_json::from_slice(resp.body()).unwrap()
                })
                .await
                .unwrap();
            if service.deployment.is_some() {
                break;
            }
        });

        proxy_client
            .request(
                Request::get("/hello")
                    .header("Host", "matrix.shuttleapp.rs")
                    .body(Body::empty())
                    .unwrap(),
            )
            .map_ok(|resp| {
                assert_eq!(resp.status(), StatusCode::OK);
                assert_eq!(
                    String::from_utf8(resp.into_body()).unwrap().as_str(),
                    "Hello, world!"
                );
            })
            .await
            .unwrap();
        // === deployment test END ===

        api_client
            .request(
                Request::delete("/projects/matrix")
                    .with_header(&authorization)
                    .body(Body::empty())
                    .unwrap(),
            )
            .map_ok(|resp| assert_eq!(resp.status(), StatusCode::OK))
            .await
            .unwrap();

        timed_loop!(wait: 1, max: 12, {
            let project: Project = api_client
                .request(
                    Request::get("/projects/matrix")
                        .with_header(&authorization)
                        .body(Body::empty())
                        .unwrap(),
                )
                .map_ok(|resp| {
                    assert_eq!(resp.status(), StatusCode::OK);
                    serde_json::from_slice(resp.body()).unwrap()
                })
                .await
                .unwrap();
            if matches!(project, Project::Destroyed(_)) {
                break;
            }
        });
    }
}
