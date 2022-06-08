// TODO: ~~user creation endpoint~~
// TODO: ~~refactor API crate to only accept local and remove auth~~
// TODO: ~~API crate should use shared secret for its control plane~~
// TODO: ~~API crate should expose the active deployed port for a service~~
// TODO: ~~gateway crate should poll active deployment port for proxy~~
// TODO: ~~gateway crate should rewrite the projects -> services route~~
// TODO: client should create project then push new deployment (refactor endpoint)
// TODO: ~~rename API crate~~
// TODO: ~~move common things to the common crate~~
// TODO: ~~AccountName and ProjectName validation logic?~~
// TODO: Add some tests (ideas?)
// TODO: Implement the delete project endpoint to make sure users can
//       self-serve out of issues
// TODO: Do a `docker pull` of the target runtime image to use when
//       starting up

#![allow(warnings)]

#[macro_use]
extern crate async_trait;

#[macro_use]
extern crate log;
extern crate core;

use std::error::Error as StdError;
use std::fmt::Formatter;
use std::io;
use std::net::IpAddr;
use std::str::FromStr;
use std::sync::Arc;
use std::convert::Infallible;

use axum::Json;
use axum::http::StatusCode;
use axum::response::{
    IntoResponse,
    Response
};
use bollard::Docker;
use convert_case::{Casing, Case};
use serde::{
    Deserialize,
    Deserializer,
    Serialize
};
use serde_json::json;
use clap::Parser;

use crate::api::make_api;
use crate::proxy::make_proxy;
use crate::service::GatewayService;
use crate::args::Args;

pub mod api;
pub mod project;
pub mod proxy;
pub mod service;
pub mod auth;
pub mod args;

#[derive(Debug)]
pub enum ErrorKind {
    KeyMissing,
    BadHost,
    KeyMalformed,
    Unauthorized,
    UserNotFound,
    ProjectNotFound,
    InvalidProjectName,
    InvalidOperation,
    Internal
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
    source: Option<Box<dyn StdError + Sync + Send + 'static>>
}

impl Error {
    pub fn source<E: StdError + Sync + Send + 'static>(kind: ErrorKind, err: E) -> Self {
        Self {
            kind,
            source: Some(Box::new(err))
        }
    }

    pub fn custom<S: AsRef<str>>(kind: ErrorKind, message: S) -> Self {
        Self {
            kind,
            source: Some(Box::new(io::Error::new(io::ErrorKind::Other, message.as_ref().to_string())))
        }
    }

    pub fn kind(kind: ErrorKind) -> Self {
        Self {
            kind,
            source: None
        }
    }
}

impl From<ErrorKind> for Error {
    fn from(kind: ErrorKind) -> Self {
        Self { kind, source: None }
    }
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        let (status, error_message) = match self.kind {
            ErrorKind::Internal => (StatusCode::INTERNAL_SERVER_ERROR, "internal server error"),
            ErrorKind::KeyMissing => (StatusCode::BAD_REQUEST, "request is missing a key"),
            ErrorKind::KeyMalformed => (StatusCode::BAD_REQUEST, "request has an invalid key"),
            ErrorKind::BadHost => (StatusCode::BAD_REQUEST, "the 'Host' header is invalid"),
            ErrorKind::UserNotFound => (StatusCode::NOT_FOUND, "user not found"),
            ErrorKind::ProjectNotFound => (StatusCode::NOT_FOUND, "project not found"),
            ErrorKind::InvalidProjectName => (StatusCode::BAD_REQUEST, "invalid project name"),
            ErrorKind::InvalidOperation => (StatusCode::BAD_REQUEST, "the requested operation is invalid"),
            ErrorKind::Unauthorized => (StatusCode::UNAUTHORIZED, "unauthorized"),
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
pub struct ProjectName(pub String);

impl<'de> Deserialize<'de> for ProjectName {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>
    {
        String::deserialize(deserializer)?
            .parse()
            .map_err(|_err| todo!())
    }
}

impl FromStr for ProjectName {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let re = regex::Regex::new("^[a-zA-Z0-9\\-_]{3,64}$").unwrap();
        if re.is_match(s) {
            Ok(Self(s.to_string()))
        } else {
            Err(Error::kind(ErrorKind::InvalidProjectName))
        }
    }
}

impl std::fmt::Display for ProjectName {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Debug, Clone, sqlx::Type, Serialize)]
#[sqlx(transparent)]
pub struct AccountName(pub String);

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
            D: Deserializer<'de>
    {
        String::deserialize(deserializer)?
            .parse()
            .map_err(|_err| todo!())
    }
}

pub trait Context<'c>: Send + Sync {
    fn docker(&self) -> &'c Docker;

    fn args(&self) -> &'c Args;
}

#[async_trait]
pub trait Service<'c> {
    type Context: Context<'c>;

    type State: EndState<'c>;

    type Error: StdError;

    /// Asks for the latest available context for task execution
    fn context(&'c self) -> Self::Context;

    /// Commit a state update to persistence
    async fn update(&self, state: &Self::State) -> Result<(), Self::Error>;
}

/// A generic state which can, when provided with a [`Context`], do
/// some work and advance itself
#[async_trait]
pub trait State<'c>: Send {
    type Next: State<'c>;

    type Error: StdError;

    async fn next<C: Context<'c>>(self, ctx: &C) -> Result<Self::Next, Self::Error>;
}

/// A [`State`] which contains all its transitions, including
/// failures
pub trait EndState<'c>
where
    Self: State<'c, Error = Infallible, Next = Self>
{
    fn is_done(&self) -> bool;
}

pub trait IntoEndState<'c, E>
where
    E: EndState<'c>
{
    fn into_end_state(self) -> Result<E, Infallible>;
}

impl<'c, E, S, Err> IntoEndState<'c, E> for Result<S, Err>
where
    E: EndState<'c> + From<S> + From<Err>
{
    fn into_end_state(self) -> Result<E, Infallible> {
        self.map(|s| E::from(s))
            .or_else(|err| Ok(E::from(err)))
    }
}

#[async_trait]
pub trait Refresh: Sized {
    type Error: StdError;

    async fn refresh<'c, C: Context<'c>>(self, ctx: &C) -> Result<Self, Self::Error>;
}

#[tokio::main]
async fn main() -> io::Result<()> {
    env_logger::init();

    let args = Args::parse();

    let gateway = GatewayService::init(args.clone()).await;

    let api = make_api(Arc::clone(&gateway));

    let api_handle = tokio::spawn(
        axum::Server::bind(&args.control)
            .serve(api.into_make_service())
    );

    let proxy = make_proxy(gateway);

    let proxy_handle = tokio::spawn(
        hyper::Server::bind(&args.user)
            .serve(proxy)
    );

    tokio::join!(api_handle, proxy_handle);

    Ok(())
}
