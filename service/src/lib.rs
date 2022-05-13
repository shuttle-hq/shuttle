#![doc(
    html_logo_url = "https://raw.githubusercontent.com/getsynth/shuttle/main/resources/logo-square-transparent.png",
    html_favicon_url = "https://raw.githubusercontent.com/getsynth/shuttle/main/resources/favicon.ico"
)]
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
//! shuttle-service = { version = "0.3", features = ["web-rocket"] }
//! ```
//!
//! and make sure your crate has a `cdylib` output target:
//!
//! ```toml
//! [lib]
//! crate-type = ["cdylib"]
//! ```
//!
//! See the [shuttle_service::main][main] macro for more information on supported services - like Axum. Here's a simple example using [rocket](https://docs.rs/rocket) to get you started:
//!
//! ```rust,no_run
//! #[macro_use]
//! extern crate rocket;
//!
//! use shuttle_service::ShuttleRocket;
//!
//! #[get("/hello")]
//! fn hello() -> &'static str {
//!     "Hello, world!"
//! }
//!
//! #[shuttle_service::main]
//! async fn init() -> ShuttleRocket {
//!     let rocket = rocket::build().mount("/", routes![hello]);
//!
//!     Ok(rocket)
//! }
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
//! Here is a quick example to deploy a service which uses a postgres database and [sqlx](http://docs.rs/sqlx):
//!
//! Depend on `shuttle-service` in `Cargo.toml`:
//!
//! ```toml
//! shuttle-service = { version = "0.3", features = ["web-rocket", "sqlx-postgres"] }
//! ```
//!
//! ```rust,no_run
//! #[macro_use]
//! extern crate rocket;
//!
//! use rocket::State;
//! use sqlx::PgPool;
//! use shuttle_service::ShuttleRocket;
//!
//! struct MyState(PgPool);
//!
//! #[get("/hello")]
//! fn hello(state: &State<MyState>) -> &'static str {
//!     // Do things with `state.0`...
//!     "Hello, Postgres!"
//! }
//!
//! #[shuttle_service::main]
//! async fn rocket(pool: PgPool) -> ShuttleRocket {
//!     let state = MyState(pool);
//!     let rocket = rocket::build().manage(state).mount("/", routes![hello]);
//!
//!     Ok(rocket)
//! }
//! ```
//!
//! To learn more about shuttle managed services, see [shuttle_service::main][main#getting-shuttle-managed-services].
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
//! Alternatively, you can override the project name on the command-line, by passing the --name argument:
//!
//! ```bash
//! cargo shuttle deploy --name=$PROJECT_NAME
//! ```
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

use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;

use async_trait::async_trait;

// Pub uses by `codegen`
pub use log;
pub use logger::Logger;
pub use tokio::runtime::Runtime;

pub mod database;

pub mod error;
pub use error::Error;

pub mod logger;

#[cfg(feature = "secrets")]
pub mod secrets;
#[cfg(feature = "secrets")]
pub use secrets::SecretStore;

#[cfg(feature = "codegen")]
extern crate shuttle_codegen;
#[cfg(feature = "codegen")]
/// Helper macro that generates the entrypoint required by any service - likely the only macro you need in this crate.
///
/// # Without shuttle managed services
/// The simplest usage is when your service does not require any shuttle managed resources, so you only need to return a shuttle supported service:
///
/// ```rust,no_run
/// use shuttle_service::ShuttleRocket;
///
/// #[shuttle_service::main]
/// async fn rocket() -> ShuttleRocket {
///     let rocket = rocket::build();
///
///     Ok(rocket)
/// }
/// ```
///
/// ## shuttle supported services
/// The following type can take the place of the `Ok` type and enjoy first class service support in shuttle. Be sure to also enable the feature on
/// `shuttle-service` in `Cargo.toml` for the type to be recognized.
///
/// | Ok type                                                                        | Feature flag | Service                                     | Version    | Example                                                                             |
/// | ------------------------------------------------------------------------------ | ------------ | ------------------------------------------- | ---------- | ----------------------------------------------------------------------------------- |
/// | [`Rocket<Build>`](https://docs.rs/rocket/0.5.0-rc.1/rocket/struct.Rocket.html) | web-rocket   | [rocket](https://docs.rs/rocket/0.5.0-rc.1) | 0.5.0-rc.1 | [GitHub](https://github.com/getsynth/shuttle/tree/main/examples/rocket/hello-world) |
/// | [`SyncWrapper<Router>`](https://docs.rs/axum/0.5/axum/struct.Router.html)      | web-axum     | [axum](https://docs.rs/axum/0.5)            | 0.5        | [GitHub](https://github.com/getsynth/shuttle/tree/main/examples/axum/hello-world)   |
/// | [`Server<T>`](https://docs.rs/tide/latest/tide/struct.Server.html)             | web-tide     | [tide](https://docs.rs/tide/0.16.0)         | 0.16.0     | [GitHub](https://github.com/getsynth/shuttle/tree/main/examples/tide/hello-world)   |
///
/// # Getting shuttle managed services
/// The shuttle is able to manage service dependencies for you. These services are passed in as inputs to your main function:
/// ```rust,no_run
/// use sqlx::PgPool;
/// use shuttle_service::ShuttleRocket;
///
/// struct MyState(PgPool);
///
/// #[shuttle_service::main]
/// async fn rocket(pool: PgPool) -> ShuttleRocket {
///     let state = MyState(pool);
///     let rocket = rocket::build().manage(state);
///
///     Ok(rocket)
/// }
/// ```
///
/// ## shuttle managed dependencies
/// The following dependencies can be managed by shuttle - remember to enable their feature flags for the `shuttle-service` dependency in `Cargo.toml`:
///
/// | Argument type                                                 | Feature flag  | Dependency                                                         | Example                                                                          |
/// | ------------------------------------------------------------- | ------------- | ------------------------------------------------------------------ | -------------------------------------------------------------------------------- |
/// | [`PgPool`](https://docs.rs/sqlx/latest/sqlx/type.PgPool.html) | sqlx-postgres | A PostgresSql instance accessed using [sqlx](https://docs.rs/sqlx) | [GitHub](https://github.com/getsynth/shuttle/tree/main/examples/rocket/postgres) |
pub use shuttle_codegen::main;
use tokio::task::JoinHandle;

#[cfg(feature = "loader")]
pub mod loader;

/// Factories can be used to request the provisioning of additional resources (like databases).
///
/// An instance of factory is passed by the deployer as an argument to [Service::build][Service::build] in the initial phase of deployment.
///
/// Also see the [declare_service!][declare_service] macro.
#[async_trait]
pub trait Factory: Send + Sync {
    /// Declare that the [Service][Service] requires a Postgres database.
    ///
    /// Returns the connection string to the provisioned database.
    async fn get_sql_connection_string(
        &mut self,
        db_type: database::Type,
    ) -> Result<String, crate::Error>;
}

/// Used to get resources of type `T` from factories.
///
/// This is mainly meant for consumption by our code generator and should generally not be implemented by users.
/// Some resources cannot cross the boundary between the api runtime and the runtime of services. These resources
/// should be created on the passed in runtime.
#[async_trait]
pub trait GetResource<T> {
    async fn get_resource(self, runtime: &Runtime) -> Result<T, crate::Error>;
}

/// Get an `sqlx::PgPool` from any factory
#[cfg(feature = "sqlx-postgres")]
#[async_trait]
impl GetResource<sqlx::PgPool> for &mut dyn Factory {
    async fn get_resource(self, runtime: &Runtime) -> Result<sqlx::PgPool, crate::Error> {
        use error::CustomError;

        let connection_string = self
            .get_sql_connection_string(database::Type::Shared)
            .await?;

        // A sqlx Pool cannot cross runtime boundaries, so make sure to create the Pool on the service end
        let pool = runtime
            .spawn(async move {
                sqlx::postgres::PgPoolOptions::new()
                    .min_connections(1)
                    .max_connections(5)
                    .connect(&connection_string)
                    .await
            })
            .await
            .map_err(CustomError::new)?
            .map_err(CustomError::new)?;

        Ok(pool)
    }
}

/// A tokio handle the service was started on
pub type ServeHandle = JoinHandle<Result<(), anyhow::Error>>;

/// The core trait of the shuttle platform. Every crate deployed to shuttle needs to implement this trait.
///
/// Use the [declare_service!][crate::declare_service] macro to expose your implementation to the deployment backend.
#[async_trait]
pub trait Service: Send + Sync {
    /// This function is run exactly once on each instance of a deployment, prior to calling [bind][Service::bind].
    ///
    /// The passed [Factory][Factory] can be used to configure additional resources (like databases).
    /// And the logger is for logging all runtime events
    ///
    /// The default is a noop that returns `Ok(())`.
    async fn build(&mut self, _: &mut dyn Factory, _logger: Logger) -> Result<(), Error> {
        Ok(())
    }

    /// This function is run exactly once on each instance of a deployment.
    ///
    /// The deployer expects this instance of [Service][Service] to bind to the passed [SocketAddr][SocketAddr].
    fn bind(&mut self, addr: SocketAddr) -> Result<ServeHandle, error::Error>;
}

/// A convenience trait for handling out of the box conversions into [Service][Service] instances.
pub trait IntoService {
    /// The [Service][Service] instance this converts to.
    type Service: Service;

    /// Convert into a [Service][Service] instance.
    fn into_service(self) -> Self::Service;
}

pub type StateBuilder<T> =
    for<'a> fn(
        &'a mut dyn Factory,
        &'a Runtime,
        Logger,
    ) -> Pin<Box<dyn Future<Output = Result<T, Error>> + Send + 'a>>;

/// A wrapper that takes a user's future, gives the future a factory, and takes the returned service from the future
/// The returned service will be deployed by shuttle
pub struct SimpleService<T> {
    service: Option<T>,
    builder: Option<StateBuilder<T>>,
    runtime: Runtime,
}

impl<T> IntoService
    for for<'a> fn(
        &'a mut dyn Factory,
        &'a Runtime,
        Logger,
    ) -> Pin<Box<dyn Future<Output = Result<T, Error>> + Send + 'a>>
where
    SimpleService<T>: Service,
{
    type Service = SimpleService<T>;

    fn into_service(self) -> Self::Service {
        SimpleService {
            service: None,
            builder: Some(self),
            runtime: Runtime::new().unwrap(),
        }
    }
}

#[cfg(feature = "web-rocket")]
#[async_trait]
impl Service for SimpleService<rocket::Rocket<rocket::Build>> {
    async fn build(&mut self, factory: &mut dyn Factory, logger: Logger) -> Result<(), Error> {
        if let Some(builder) = self.builder.take() {
            let rocket = builder(factory, &self.runtime, logger).await?;

            self.service = Some(rocket);
        }

        Ok(())
    }

    fn bind(&mut self, addr: SocketAddr) -> Result<ServeHandle, error::Error> {
        let rocket = self.service.take().expect("service has already been bound");

        let config = rocket::Config {
            address: addr.ip(),
            port: addr.port(),
            log_level: rocket::config::LogLevel::Off,
            ..Default::default()
        };
        let launched = rocket.configure(config).launch();
        let handle = self.runtime.spawn(async {
            let _rocket = launched.await.map_err(error::CustomError::new)?;

            Ok(())
        });
        Ok(handle)
    }
}

#[allow(dead_code)]
#[cfg(feature = "web-rocket")]
pub type ShuttleRocket = Result<rocket::Rocket<rocket::Build>, Error>;

#[cfg(feature = "web-axum")]
#[async_trait]
impl Service for SimpleService<sync_wrapper::SyncWrapper<axum::Router>> {
    async fn build(&mut self, factory: &mut dyn Factory, logger: Logger) -> Result<(), Error> {
        if let Some(builder) = self.builder.take() {
            let axum = builder(factory, &self.runtime, logger).await?;

            self.service = Some(axum);
        }

        Ok(())
    }

    fn bind(&mut self, addr: SocketAddr) -> Result<ServeHandle, error::Error> {
        let axum = self
            .service
            .take()
            .expect("service has already been bound")
            .into_inner();

        let handle = self.runtime.spawn(async move {
            axum::Server::bind(&addr)
                .serve(axum.into_make_service())
                .await
                .map_err(error::CustomError::new)
        });

        Ok(handle)
    }
}

#[allow(dead_code)]
#[cfg(feature = "web-axum")]
pub type ShuttleAxum = Result<sync_wrapper::SyncWrapper<axum::Router>, Error>;

#[cfg(feature = "web-tide")]
#[async_trait]
impl<T> Service for SimpleService<tide::Server<T>>
where
    T: Clone + Send + Sync + 'static,
{
    async fn build(&mut self, factory: &mut dyn Factory, logger: Logger) -> Result<(), Error> {
        if let Some(builder) = self.builder.take() {
            let tide = builder(factory, &self.runtime, logger).await?;

            self.service = Some(tide);
        }

        Ok(())
    }

    fn bind(&mut self, addr: SocketAddr) -> Result<ServeHandle, error::Error> {
        let tide = self.service.take().expect("service has already been bound");

        let handle = self
            .runtime
            .spawn(async move { tide.listen(addr).await.map_err(error::CustomError::new) });

        Ok(handle)
    }
}

#[allow(dead_code)]
#[cfg(feature = "web-tide")]
pub type ShuttleTide<T> = Result<tide::Server<T>, Error>;

#[cfg(feature = "web-tower")]
#[async_trait]
impl<T> Service for SimpleService<T>
where
    T: tower::Service<hyper::Request<hyper::Body>, Response = hyper::Response<hyper::Body>>
        + Clone
        + Send
        + Sync
        + 'static,
    T::Error: std::error::Error + Send + Sync,
    T::Future: std::future::Future + Send + Sync,
{
    async fn build(
        &mut self,
        factory: &mut dyn Factory,
        logger: logger::Logger,
    ) -> Result<(), Error> {
        if let Some(builder) = self.builder.take() {
            let tower = builder(factory, &self.runtime, logger).await?;

            self.service = Some(tower);
        }

        Ok(())
    }

    fn bind(&mut self, addr: SocketAddr) -> Result<ServeHandle, error::Error> {
        let service = self.service.take().expect("service has already been bound");

        let handle = self.runtime.spawn(async move {
            let shared = tower::make::Shared::new(service);
            hyper::Server::bind(&addr)
                .serve(shared)
                .await
                .map_err(error::CustomError::new)?;

            Ok(())
        });

        Ok(handle)
    }
}

/// Helper macro that generates the entrypoint required of any service.
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
