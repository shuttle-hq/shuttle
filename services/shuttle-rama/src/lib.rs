#![doc = include_str!("../README.md")]

use rama::{
    error::OpaqueError,
    http::{server::HttpServer, service::web::response::IntoResponse, Request},
    tcp::server::TcpListener,
    Service,
};
use shuttle_runtime::tokio;
use std::{convert::Infallible, fmt, net::SocketAddr};

/// A wrapper type for [`Service`] so we can implement [`shuttle_runtime::Service`] for it.
pub struct RamaService<T, State> {
    svc: T,
    state: State,
}

impl<T: Clone, State: Clone> Clone for RamaService<T, State> {
    fn clone(&self) -> Self {
        Self {
            svc: self.svc.clone(),
            state: self.state.clone(),
        }
    }
}

impl<T: fmt::Debug, State: fmt::Debug> fmt::Debug for RamaService<T, State> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RamaService")
            .field("svc", &self.svc)
            .field("state", &self.state)
            .finish()
    }
}

/// Private type wrapper to indicate [`RamaService`]
/// is used by the user from the Transport layer (tcp).
pub struct Transport<S>(S);

/// Private type wrapper to indicate [`RamaService`]
/// is used by the user from the Application layer (http(s)).
pub struct Application<S>(S);

macro_rules! impl_wrapper_derive_traits {
    ($name:ident) => {
        impl<S: Clone> Clone for $name<S> {
            fn clone(&self) -> Self {
                Self(self.0.clone())
            }
        }

        impl<S: fmt::Debug> fmt::Debug for $name<S> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.debug_tuple(stringify!($name)).field(&self.0).finish()
            }
        }
    };
}

impl_wrapper_derive_traits!(Transport);
impl_wrapper_derive_traits!(Application);

impl<S> RamaService<Transport<S>, ()> {
    pub fn transport(svc: S) -> Self {
        Self {
            svc: Transport(svc),
            state: (),
        }
    }
}

impl<S> RamaService<Application<S>, ()> {
    pub fn application(svc: S) -> Self {
        Self {
            svc: Application(svc),
            state: (),
        }
    }
}

impl<T> RamaService<T, ()> {
    /// Attach state to this [`RamaService`], such that it will be passed
    /// as part of each request's [`Context`].
    ///
    /// [`Context`]: rama::Context
    pub fn with_state<State>(self, state: State) -> RamaService<T, State>
    where
        State: Clone + Send + Sync + 'static,
    {
        RamaService {
            svc: self.svc,
            state,
        }
    }
}

#[shuttle_runtime::async_trait]
impl<S, State> shuttle_runtime::Service for RamaService<Transport<S>, State>
where
    S: Service<State, tokio::net::TcpStream>,
    State: Clone + Send + Sync + 'static,
{
    /// Takes the service that is returned by the user in their [shuttle_runtime::main] function
    /// and binds to an address passed in by shuttle.
    async fn bind(self, addr: SocketAddr) -> Result<(), shuttle_runtime::BoxDynError> {
        TcpListener::build_with_state(self.state)
            .bind(addr)
            .await?
            .serve(self.svc.0)
            .await;
        Ok(())
    }
}

#[shuttle_runtime::async_trait]
impl<S, State, Response> shuttle_runtime::Service for RamaService<Application<S>, State>
where
    S: Service<State, Request, Response = Response, Error = Infallible>,
    Response: IntoResponse + Send + 'static,
    State: Clone + Send + Sync + 'static,
{
    /// Takes the service that is returned by the user in their [shuttle_runtime::main] function
    /// and binds to an address passed in by shuttle.
    async fn bind(self, addr: SocketAddr) -> Result<(), shuttle_runtime::BoxDynError> {
        // shuttle only supports h1 between load balancer <=> web service,
        // h2 is terminated by shuttle's load balancer
        HttpServer::http1()
            .listen_with_state(self.state, addr, self.svc.0)
            .await?;
        Ok(())
    }
}

#[doc = include_str!("../README.md")]
pub type ShuttleRamaTransport<S, State = ()> =
    Result<RamaService<Transport<S>, State>, shuttle_runtime::BoxDynError>;

#[doc = include_str!("../README.md")]
pub type ShuttleRamaApplication<S, State = ()> =
    Result<RamaService<Application<S>, State>, shuttle_runtime::BoxDynError>;
