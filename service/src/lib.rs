#![doc(html_logo_url = "https://raw.githubusercontent.com/getsynth/shuttle/main/resources/logo-square-transparent.png", html_favicon_url = "https://raw.githubusercontent.com/getsynth/shuttle/main/resources/favicon.ico")]
//! # Shuttle - Deploy Rust apps with a single Cargo subcommand
//! <div style="display: flex; margin-top: 30px; margin-bottom: 30px;">
//! <img src="https://raw.githubusercontent.com/getsynth/shuttle/main/resources/logo-rectangle-transparent.png" width="400px" style="margin-left: auto; margin-right: auto;"/>
//! </div>
//!
//! Hello, and welcome to the <span style="font-family: Sans-Serif;"><a href="https://shuttle.rs">shuttle</a></span> API documentation!
//!
//! Shuttle is an open-source app platform that uses traits and annotations to configure your backend deployments.
//!
//! ## Usage
//!
//! Depend on `shuttle-service` in `Cargo.toml`:
//!
//! ```toml
//! shuttle-service = "0.2"
//! ```
//!
//! and make sure your crate has a `cdylib` output target:
//!
//! ```toml
//! [lib]
//! crate-type = ["cdylib"]
//! ```
//!
//! See the [declare_service!][declare_service] macro for more information on how to implement a service. Here's a simple example using [rocket][rocket] to get you started:
//!
//! ```rust,no_run
//! #[macro_use]
//! extern crate shuttle_service;
//!
//! #[macro_use]
//! extern crate rocket;
//!
//! use rocket::{Build, Rocket};
//!
//! #[get("/hello")]
//! fn hello() -> &'static str {
//!     "Hello, world!"
//! }
//!
//! fn init() -> Rocket<Build> {
//!     rocket::build().mount("/", routes![hello])
//! }
//!
//! declare_service!(Rocket<Build>, init);
//! ```
//!
//! Complete examples can be found [in the repository](https://github.com/getsynth/shuttle/tree/main/examples/rocket).
//!
//! ## Deploying
//!
//! You can deploy your service with the [`cargo shuttle`](https://docs.rs/crate/cargo-shuttle/latest) subcommand. To install run:
//!
//! ```bash
//! $ cargo install cargo-shuttle
//! ```
//!
//! in a terminal. Once installed, run:
//!
//! ```bash
//! $ cargo shuttle login
//! ```
//!
//! this will open a browser window and prompt you to connect using your GitHub account.
//!
//! Then, deploy the service with:
//!
//! ```bash
//! $ cargo shuttle deploy
//! ```
//!
//! Your service will immediately be available at `{crate_name}.shuttleapp.rs`. For example:
//!
//! ```bash
//! $ curl https://hello-world-rocket-app.shuttleapp.rs
//! Hello, world!
//! ```
//!
//! ## Using `sqlx`
//!
//! Here is a quick example to deploy a service which uses a postgres database and [sqlx][sqlx]:
//!
//! ```rust,no_run
//! #[macro_use]
//! extern crate shuttle_service;
//! use shuttle_service::{Factory, Error};
//!
//! #[macro_use]
//! extern crate rocket;
//! use rocket::{Rocket, Build, State};
//!
//! use sqlx::PgPool;
//!
//! struct MyState(PgPool);
//!
//! #[get("/hello")]
//! fn hello(state: &State<MyState>) -> &'static str {
//!     // Do things with `state.0`...
//!     "Hello, Postgres!"
//! }
//!
//! async fn state(factory: &mut dyn Factory) -> Result<MyState, shuttle_service::Error> {
//!    let pool = sqlx::postgres::PgPoolOptions::new()
//!        .connect(&factory.get_sql_connection_string().await?)
//!        .await?;
//!    Ok(MyState(pool))
//! }
//!
//! fn rocket() -> Rocket<Build> {
//!     rocket::build().mount("/", routes![hello])
//! }
//!
//! declare_service!(Rocket<Build>, rocket, state);
//! ```
//!
//! To learn more about how to build services with states, and services that require additional resources, see [Factory][Factory].
//!
//! ## Configuration
//!
//! The `cargo shuttle` command can be customised by creating a `Shuttle.toml` in the same location as your `Cargo.toml`.
//!
//! ## Getting API keys
//!
//! After you've installed the [cargo-shuttle](https://docs.rs/crate/cargo-shuttle/latest) command, run:
//!
//! ```bash
//! $ cargo shuttle login
//! ```
//!
//! this will open a browser window and prompt you to connect using your GitHub account.
//!
//! ##### Change the name of your service
//!
//! To have your service deployed with a different name, add a `name` entry in the `Shuttle.toml`:
//!
//! ```toml
//! name = "hello-world"
//! ```
//!
//! If the `name` key is not specified, the service's name will be the same as the crate's name.
//!
//! ## We're in alpha 🤗
//!
//! Thanks for using shuttle! We're very happy to have you with us!
//!
//! During our alpha period, API keys are completely free and you can deploy as many services as you want.
//!
//! Just keep in mind that there may be some kinks that require us to take all deployments down once in a while. In certain circumstances we may also have to delete all the data associated with those deployments.
//!
//! To stay updated with the release status of shuttle, [join our Discord](https://discord.gg/H33rRDTm3p)!
//!
//! ## Join Discord
//!
//! If you have any questions, [join our Discord server](https://discord.gg/H33rRDTm3p). There's always someone on there that can help!
//!
//! You can also [open an issue or a discussion on GitHub](https://github.com/getsynth/shuttle).
//!

use async_trait::async_trait;
use std::future::Future;

pub use rocket;
use rocket::{Build, Rocket};

use tokio::runtime::Runtime;

use sqlx::PgPool;

use std::net::SocketAddr;
use std::pin::Pin;

pub mod error;
pub use error::Error;

/// Factories can be used to request the provisioning of additional resources (like databases).
///
/// An instance of factory is passed by the deployer as an argument to [Service::build][Service::build] in the initial phase of deployment.
///
/// Also see the [declare_service!][declare_service] macro.
#[async_trait]
pub trait Factory: Send + Sync {
    /// Declare that the [Service][Service] requires a postgres database.
    ///
    /// Returns the connection string to the provisioned database.
    async fn get_sql_connection_string(&mut self) -> Result<String, crate::Error>;

    async fn get_postgres_connection_pool(&mut self) -> Result<PgPool, crate::Error> {
        let connection_string = self.get_sql_connection_string().await?;
        let pool = sqlx::postgres::PgPoolOptions::new()
            .min_connections(1)
            .max_connections(5)
            .connect(&connection_string)
            .await?;

        Ok(pool)
    }
}

/// The core trait of the shuttle platform. Every crate deployed to shuttle needs to implement this trait.
///
/// Use the [declare_service!][crate::declare_service] macro to expose your implementation to the deployment backend.
#[async_trait]
pub trait Service: Send + Sync {
    /// This function is run exactly once on each instance of a deployment, prior to calling [bind][Service::bind].
    ///
    /// The passed [Factory][Factory] can be used to configure additional resources (like databases).
    ///
    /// The default is a noop that returns `Ok(())`.
    fn build(&mut self, _: &mut dyn Factory) -> Result<(), Error> {
        Ok(())
    }

    /// This function is run exactly once on each instance of a deployment.
    ///
    /// The deployer expects this instance of [Service][Service] to bind to the passed [SocketAddr][SocketAddr].
    fn bind(&mut self, addr: SocketAddr) -> Result<(), error::Error>;
}

/// A convenience trait for handling out of the box conversions into [Service][Service] instances.
pub trait IntoService {
    /// The [Service][Service] instance this converts to.
    type Service: Service;

    /// Convert into a [Service][Service] instance.
    fn into_service(self) -> Self::Service;
}

/// A convenience struct for building a [Service][Service] from a [Rocket<Build>][Rocket] instance.
///
/// To construct, use [into_service][IntoService::into_service].
///
/// If you have a state of type `T` you wish to load rocket with, use [into_service][IntoService::into_service] on a pair
/// `(Rocket<Build>, async fn (&mut dyn Factory) -> Result<T, Error>)`.
///
/// Also see the [declare_service!][declare_service] macro.
pub struct RocketService<T: Sized> {
    rocket: Option<Rocket<Build>>,
    state_builder:
    Option<fn(&mut dyn Factory) -> Pin<Box<dyn Future<Output=Result<T, Error>> + Send + '_>>>,
    runtime: Runtime,
}

impl IntoService for Rocket<Build> {
    type Service = RocketService<()>;
    fn into_service(self) -> Self::Service {
        RocketService {
            rocket: Some(self),
            state_builder: None,
            runtime: Runtime::new().unwrap(),
        }
    }
}

impl<T: Send + Sync + 'static> IntoService
for (
    Rocket<Build>,
    fn(&mut dyn Factory) -> Pin<Box<dyn Future<Output=Result<T, Error>> + Send + '_>>,
)
{
    type Service = RocketService<T>;

    fn into_service(self) -> Self::Service {
        RocketService {
            rocket: Some(self.0),
            state_builder: Some(self.1),
            runtime: Runtime::new().unwrap(),
        }
    }
}

#[async_trait]
impl<T> Service for RocketService<T>
    where
        T: Send + Sync + 'static,
{
    fn build(&mut self, factory: &mut dyn Factory) -> Result<(), Error> {
        if let Some(state_builder) = self.state_builder.take() {
            // We want to build any sqlx pools on the same runtime the client code will run on. Without this expect to get errors of no tokio reactor being present.
            let state = self.runtime.block_on(state_builder(factory))?;

            if let Some(rocket) = self.rocket.take() {
                self.rocket.replace(rocket.manage(state));
            }
        }

        Ok(())
    }

    fn bind(&mut self, addr: SocketAddr) -> Result<(), error::Error> {
        let rocket = self.rocket.take().expect("service has already been bound");

        let config = rocket::Config {
            address: addr.ip(),
            port: addr.port(),
            log_level: rocket::config::LogLevel::Normal,
            ..Default::default()
        };
        let launched = rocket.configure(config).launch();
        self.runtime.block_on(launched)?;
        Ok(())
    }
}

/// Helper macro that generates the entrypoint required of any service. Likely the only macro you need in this crate.
///
/// Can be used in one of two ways:
///
/// ## Without a state
///
/// If your service does not require a state (like a database connection pool), just pass a type and a constructor function:
///
/// ```rust,no_run
/// #[macro_use]
/// extern crate shuttle_service;
///
/// use rocket::{Rocket, Build};
///
/// fn rocket() -> Rocket<Build> {
///     rocket::build()
/// }
///
/// declare_service!(Rocket<Build>, rocket);
/// ```
///
/// The constructor function must return an instance of the type passed as first argument. Furthermore, the type must implement [IntoService][IntoService].
///
/// ## With a state
///
/// If your service requires a state, pass a type, a constructor and a state builder:
///
/// ```rust,no_run
/// use rocket::{Rocket, Build};
/// use sqlx::PgPool;
///
/// #[macro_use]
/// extern crate shuttle_service;
/// use shuttle_service::{Factory, Error};
///
/// struct MyState(PgPool);
///
/// async fn state(factory: &mut dyn Factory) -> Result<MyState, shuttle_service::Error> {
///    let pool = sqlx::postgres::PgPoolOptions::new()
///        .connect(&factory.get_sql_connection_string().await?)
///        .await?;
///    Ok(MyState(pool))
/// }
///
/// fn rocket() -> Rocket<Build> {
///     rocket::build()
/// }
///
/// declare_service!(Rocket<Build>, rocket, state);
/// ```
///
/// The state builder will be called when the deployer calls [Service::build][Service::build].
///
#[macro_export]
macro_rules! declare_service {
    ($service_type:ty, $constructor:path) => {
        #[no_mangle]
        pub extern "C" fn _create_service() -> *mut dyn $crate::Service {
            // Ensure constructor returns concrete type.
            let constructor: fn() -> $service_type = $constructor;

            let obj = $crate::IntoService::into_service(constructor());
            let boxed: Box<dyn $crate::Service> = Box::new(obj);
            Box::into_raw(boxed)
        }
    };
    ($service_type:ty, $constructor:path, $state_builder:path) => {
        #[no_mangle]
        pub extern "C" fn _create_service() -> *mut dyn $crate::Service {
            // Ensure constructor returns concrete type.
            let constructor: fn() -> $service_type = $constructor;

            // Ensure state builder is a function
            let state_builder: fn(
                &mut dyn $crate::Factory,
            ) -> std::pin::Pin<
                Box<dyn std::future::Future<Output = Result<_, $crate::Error>> + Send + '_>,
            > = |factory| Box::pin($state_builder(factory));

            let obj = $crate::IntoService::into_service((constructor(), state_builder));
            let boxed: Box<dyn $crate::Service> = Box::new(obj);
            Box::into_raw(boxed)
        }
    };
}
