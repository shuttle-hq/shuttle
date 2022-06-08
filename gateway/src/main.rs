// TODO: ~~user creation endpoint~~
// TODO: ~~refactor API crate to only accept local and remove auth~~
// TODO: ~~API crate should use shared secret for its control plane~~
// TODO: ~~API crate should expose the active deployed port for a service~~
// TODO: ~~gateway crate should poll active deployment port for proxy~~
// TODO: ~~gateway crate should rewrite the projects -> services route~~
// TODO: client should create project then push new deployment (refactor endpoint)
// TODO: ~~rename API crate~~
// TODO: move common things to the common crate
// TODO: AccountName and ProjectName validation logic?

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

use crate::api::make_api;
use crate::proxy::make_proxy;
use crate::service::GatewayService;

pub mod api;
pub mod project;
pub mod proxy;
pub mod service;
pub mod auth;

pub const API_PORT: &'static str = "8001";
pub const PROXY_PORT: u16 = 8000;

#[derive(Debug)]
pub enum ErrorKind {
    KeyMissing,
    BadHost,
    KeyMalformed,
    Unauthorized,
    UserNotFound,
    Internal
}

impl std::fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let formatted = format!("{:?}", self).to_case(Case::Snake);
        write!(f, "{}", formatted)
    }
}

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
        // TODO: re correct?
        let re = regex::Regex::new("^[a-zA-Z0-9-_]{3,64}$").unwrap();
        if re.is_match(s) {
            Ok(Self(s.to_string()))
        } else {
            todo!()
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

// TODO may not want to have Refresh for variants of Project as this may drift OOS
#[async_trait]
pub trait Refresh: Sized {
    type Error: StdError;

    async fn refresh<'c, C: Context<'c>>(self, ctx: &C) -> Result<Self, Self::Error>;
}

#[tokio::main]
async fn main() -> io::Result<()> {
    env_logger::init();

    let gateway = GatewayService::init(None).await;

    let api = make_api(Arc::clone(&gateway));

    let api_handle = tokio::spawn(
        axum::Server::bind(&format!("0.0.0.0:{}", API_PORT).parse().unwrap())
            .serve(api.into_make_service())
    );

    let proxy = make_proxy(gateway);

    let proxy_handle = tokio::spawn(
        hyper::Server::bind(&format!("0.0.0.0:{}", PROXY_PORT).parse().unwrap())
            .serve(proxy)
    );

    tokio::join!(api_handle, proxy_handle);

    Ok(())
}
