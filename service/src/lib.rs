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
//! Start by installing the [`cargo shuttle`](https://docs.rs/crate/cargo-shuttle/latest) subcommand by running the following in a terminal:
//!
//! ```bash
//! $ cargo install cargo-shuttle
//! ```
//!
//! Now that shuttle is installed, you can create your first project using:
//!
//! ```bash
//! $ cargo shuttle init --rocket my-rocket-app
//! ```
//!
//! By looking at the `Cargo.toml` file of the generated `my-rocket-app` project you will see it has been made to
//! be a library crate with a `shuttle-service` dependency with the `web-rocket` feature on the `shuttle-service` dependency.
//!
//! ```toml
//! shuttle-service = { version = "0.5.1", features = ["web-rocket"] }
//! ```
//!
//! A boilerplate code for your rocket project can also be found in `src/lib.rs`:
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
//! See the [shuttle_service::main][main] macro for more information on supported services - such as `axum`.
//! Or look at more complete examples [in the repository](https://github.com/getsynth/shuttle/tree/main/examples), but
//! take note that the examples may update before official releases.
//!
//! ## Running locally
//! To test your app locally before deploying, use:
//!
//! ```bash
//! $ cargo shuttle run
//! ```
//!
//! You should see your app build and start on the default port 8000. You can test this using;
//!
//! ```bash
//! $ curl http://localhost:8000/hello
//! Hello, world!
//! ```
//!
//! ## Deploying
//!
//! You can deploy your service with the [`cargo shuttle`](https://docs.rs/crate/cargo-shuttle/latest) subcommand too.
//! But, you will need to authenticate with the shuttle service first using:
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
//! $ curl https://my-rocket-app.shuttleapp.rs/hello
//! Hello, world!
//! ```
//!
//! ## Using `sqlx`
//!
//! Here is a quick example to deploy a service that uses a postgres database and [sqlx](http://docs.rs/sqlx):
//!
//! Add the `sqlx-postgres` feature to the `shuttle-service` dependency, and add `sqlx` as a dependency with the `runtime-tokio-native-tls` and `postgres` features inside `Cargo.toml`:
//!
//! ```toml
//! shuttle-service = { version = "0.5.1", features = ["web-rocket", "sqlx-postgres"] }
//! sqlx = { version = "0.6.1", features = ["runtime-tokio-native-tls", "postgres"] }
//! ```
//!
//! Now update the `#[shuttle_service::main]` function to take in a `PgPool`:
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
//! async fn rocket(#[shared::Postgres] pool: PgPool) -> ShuttleRocket {
//!     let state = MyState(pool);
//!     let rocket = rocket::build().manage(state).mount("/", routes![hello]);
//!
//!     Ok(rocket)
//! }
//! ```
//!
//! For a local run, shuttle will automatically provision a Postgres instance inside a [Docker](https://www.docker.com/) container on your machine and connect it to the `PgPool`.
//!
//! For deploys, shuttle will provision a database for your application and connect it to the `PgPool` on your behalf.
//!
//! To learn more about shuttle managed services, see [shuttle_service::main][main#getting-shuttle-managed-services].
//!
//! ## Configuration
//!
//! The `cargo shuttle` command can be customised by creating a `Shuttle.toml` in the same location as your `Cargo.toml`.
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
//! ##### Using Podman instead of Docker
//! If you are using [Podman](https://podman.io/) instead of Docker, then `cargo shuttle run` will give
//! `got unexpected error while inspecting docker container: error trying to connect: No such file or directory` error.
//!
//! To fix this error you will need to expose a rootless socket for Podman first. This can be done using:
//!
//! ```bash
//! podman system service --time=0 unix:///tmp/podman.sock
//! ```
//!
//! Now set the `DOCKER_HOST` environment variable to point to this socket using:
//!
//! ```bash
//! export DOCKER_HOST=unix:///tmp/podman.sock
//! ```
//!
//! Now all `cargo shuttle run` commands will work against Podman.
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

pub use async_trait::async_trait;

// Pub uses by `codegen`
pub use log;
pub use tokio::runtime::Runtime;

pub mod error;
pub use error::Error;

pub mod logger;

pub use shuttle_common::database;

#[cfg(any(feature = "sqlx-postgres", feature = "mongodb-integration",))]
pub mod shared;

#[cfg(feature = "secrets")]
pub mod secrets;
#[cfg(feature = "secrets")]
pub use secrets::SecretStore;

#[cfg(any(
    feature = "sqlx-aws-mariadb",
    feature = "sqlx-aws-mysql",
    feature = "sqlx-aws-postgres"
))]
pub mod aws;

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
/// The following types can be returned from a `#[shuttle_service::main]` function and enjoy first class service support in shuttle. Be sure to also enable the correct feature on
/// `shuttle-service` in `Cargo.toml` for the type to be recognized.
///
/// | Return type                           | Feature flag | Service                                     | Version    | Example                                                                             |
/// | ------------------------------------- | ------------ | ------------------------------------------- | ---------- | ----------------------------------------------------------------------------------- |
/// | `ShuttleRocket`                       | web-rocket   | [rocket](https://docs.rs/rocket/0.5.0-rc.2) | 0.5.0-rc.2 | [GitHub](https://github.com/getsynth/shuttle/tree/main/examples/rocket/hello-world) |
/// | `ShuttleAxum`                         | web-axum     | [axum](https://docs.rs/axum/0.5)            | 0.5        | [GitHub](https://github.com/getsynth/shuttle/tree/main/examples/axum/hello-world)   |
/// | `ShuttleTide`                         | web-tide     | [tide](https://docs.rs/tide/0.16.0)         | 0.16.0     | [GitHub](https://github.com/getsynth/shuttle/tree/main/examples/tide/hello-world)   |
/// | `ShuttlePoem`                         | web-poem     | [poem](https://docs.rs/poem/1.3.35)         | 1.3.35     | [GitHub](https://github.com/getsynth/shuttle/tree/main/examples/poem/hello-world)   |
/// | `Result<T, shuttle_service::Error>`   | web-tower    | [tower](https://docs.rs/tower/0.4.12)       | 0.14.12    | [GitHub](https://github.com/getsynth/shuttle/tree/main/examples/tower/hello-world)  |
///
/// # Getting shuttle managed services
/// Shuttle is able to manage service dependencies for you. These services are passed in as inputs to your `#[shuttle_service::main]` function and are configured using attributes:
/// ```rust,no_run
/// use sqlx::PgPool;
/// use shuttle_service::ShuttleRocket;
///
/// struct MyState(PgPool);
///
/// #[shuttle_service::main]
/// async fn rocket(#[shared::Postgres] pool: PgPool) -> ShuttleRocket {
///     let state = MyState(pool);
///     let rocket = rocket::build().manage(state);
///
///     Ok(rocket)
/// }
/// ```
///
/// ## shuttle managed dependencies
/// The following dependencies can be managed by shuttle - remember to enable their feature flags for the `shuttle-service` dependency in `Cargo.toml` and configure them using an attribute annotation:
///

/// | Argument type                                                            | Feature flag        | Attribute            | Dependency                                                                                         | Example                                                                          |
/// | ------------------------------------------------------------------------ | ------------------- | -------------------- | -------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------- |
/// | [`PgPool`](https://docs.rs/sqlx/latest/sqlx/type.PgPool.html)            | sqlx-postgres       | `shared::Postgres`   | A shared PostgresSQL instance accessed using [sqlx](https://docs.rs/sqlx)                          | [GitHub](https://github.com/getsynth/shuttle/tree/main/examples/rocket/postgres) |
/// | [`Database`](https://docs.rs/mongodb/latest/mongodb/struct.Database.html)| mongodb-integration | `shared::MongoDb`    | A shared MongoDb database accessed using the [mongodb](https://docs.rs/mongodb) driver             | [GitHub](https://github.com/getsynth/shuttle/tree/main/examples/poem/mongodb)    |
/// | [`MySqlPool`](https://docs.rs/sqlx/latest/sqlx/type.MySqlPool.html)      | sqlx-aws-mariadb    | `aws::rds::MariaDB`  | An AWS RDS MariaDB instance tied to your instance and accessed using [sqlx](https://docs.rs/sqlx)  |                                                                                  |
/// | [`MySqlPool`](https://docs.rs/sqlx/latest/sqlx/type.MySqlPool.html)      | sqlx-aws-mysql      | `aws::rds::MySql`    | An AWS RDS MySql instance tied to your instance and accessed using [sqlx](https://docs.rs/sqlx)    |                                                                                  |
/// | [`PgPool`](https://docs.rs/sqlx/latest/sqlx/type.PgPool.html)            | sqlx-aws-postgres   | `aws::rds::Postgres` | An AWS RDS Postgres instance tied to your instance and accessed using [sqlx](https://docs.rs/sqlx) | [GitHub](https://github.com/getsynth/shuttle/tree/main/examples/tide/postgres)   |
pub use shuttle_codegen::main;
use tokio::task::JoinHandle;

#[cfg(feature = "loader")]
pub mod loader;

/// Factories can be used to request the provisioning of additional resources (like databases).
///
/// An instance of factory is passed by the deployer as an argument to [Service::build][Service::build] in the initial phase of deployment.
///
/// Also see the [main][main] macro.
#[async_trait]
pub trait Factory: Send + Sync {
    /// Declare that the [Service][Service] requires a database.
    ///
    /// Returns the connection string to the provisioned database.
    async fn get_db_connection_string(
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
pub trait ResourceBuilder<T> {
    fn new() -> Self;
    async fn build(self, factory: &mut dyn Factory, runtime: &Runtime) -> Result<T, crate::Error>;
}

/// A tokio handle the service was started on
pub type ServeHandle = JoinHandle<Result<(), error::Error>>;

/// The core trait of the shuttle platform. Every crate deployed to shuttle needs to implement this trait.
///
/// Use the [main][main] macro to expose your implementation to the deployment backend.
//
// TODO: our current state machine in the api crate stores this service and can move it across
// threads (handlers) causing `Service` to need `Sync`. We should remove this restriction
#[async_trait]
pub trait Service: Send + Sync {
    /// This function is run exactly once on each instance of a deployment.
    ///
    /// The deployer expects this instance of [Service][Service] to bind to the passed [SocketAddr][SocketAddr].
    async fn bind(mut self: Box<Self>, addr: SocketAddr) -> Result<(), error::Error>;
}

/// This function is generated by our codegen. It uses the factory to get other services and instantiate them on
/// the correct tokio runtime. This function also sets the runtime logger. The output is a future where `T`
/// should implement [Service].
pub type StateBuilder<T> =
    for<'a> fn(
        &'a mut dyn Factory,
        &'a Runtime,
        Box<dyn log::Log>,
    ) -> Pin<Box<dyn Future<Output = Result<T, Error>> + Send + 'a>>;

/// This function is generated by codegen to ensure binding happens on the other side of the FFI and on the correct
/// tokio runtime.
pub type Binder = for<'a> fn(Box<dyn Service>, SocketAddr, &'a Runtime) -> ServeHandle;

#[allow(dead_code)]
pub struct Bootstrapper {
    service: Option<Box<dyn Service>>,
    builder: Option<StateBuilder<Box<dyn Service>>>,
    binder: Binder,
    // Do you have time on your hands? If yes, then move this field higher and spend endless hours debugging the segmentation fault
    // It seems that the [Runtime] changes in size when crossing the FFI which misaligns all fields after it
    runtime: Option<Runtime>,
}

impl Bootstrapper {
    pub fn new(builder: StateBuilder<Box<dyn Service>>, binder: Binder, runtime: Runtime) -> Self {
        Self {
            service: None,
            builder: Some(builder),
            binder,
            runtime: Some(runtime),
        }
    }

    #[cfg(feature = "loader")]
    async fn bootstrap(
        &mut self,
        factory: &mut dyn Factory,
        logger: Box<dyn log::Log>,
    ) -> Result<(), Error> {
        if let Some(builder) = self.builder.take() {
            let service = builder(factory, self.runtime.as_ref().unwrap(), logger).await?;
            self.service = Some(service);
        }

        Ok(())
    }

    #[cfg(feature = "loader")]
    fn into_handle(mut self, addr: SocketAddr) -> Result<ServeHandle, Error> {
        let service = self.service.take().expect("service has already been bound");

        let handle = (self.binder)(service, addr, self.runtime.as_ref().unwrap());

        Ok(handle)
    }
}

impl Drop for Bootstrapper {
    fn drop(&mut self) {
        if let Some(runtime) = self.runtime.take() {
            // TODO: find a way to drop the runtime
            std::mem::forget(runtime);
        }
    }
}

#[cfg(feature = "web-rocket")]
#[async_trait]
impl Service for rocket::Rocket<rocket::Build> {
    async fn bind(mut self: Box<Self>, addr: SocketAddr) -> Result<(), error::Error> {
        let shutdown = rocket::config::Shutdown {
            ctrlc: false,
            ..rocket::config::Shutdown::default()
        };

        let config = rocket::Config {
            address: addr.ip(),
            port: addr.port(),
            log_level: rocket::config::LogLevel::Off,
            shutdown,
            ..Default::default()
        };
        let _rocket = self
            .configure(config)
            .launch()
            .await
            .map_err(error::CustomError::new)?;

        Ok(())
    }
}

#[cfg(feature = "web-rocket")]
pub type ShuttleRocket = Result<rocket::Rocket<rocket::Build>, Error>;

#[cfg(feature = "web-poem")]
#[async_trait]
impl<T> Service for T
where
    T: poem::Endpoint + Sync + Send + 'static,
{
    async fn bind(mut self: Box<Self>, addr: SocketAddr) -> Result<(), error::Error> {
        poem::Server::new(poem::listener::TcpListener::bind(addr))
            .run(self)
            .await
            .map_err(error::CustomError::new)?;

        Ok(())
    }
}

#[cfg(feature = "web-poem")]
pub type ShuttlePoem<T> = Result<T, Error>;

#[cfg(feature = "web-warp")]
#[async_trait]
impl<T> Service for T
where
    //T: Send
    //+ Sync
    //+ Clone
    //+ 'static
    //+ tower::Service<hyper::Request<hyper::Body>, Response = hyper::Response<hyper::Body>>,
    //T: Send + Sync + Clone + 'static + tower::Service<hyper::Request<hyper::Body>>,
    T: Send + Sync + Clone + 'static + warp::Filter,
    T::Extract: warp::reply::Reply,
    //T::Error: warp::reject::Reject,
    //T::Error: std::error::Error + Send + Sync,
    //T::Future: std::future::Future + Send + Sync,
{
    async fn bind(mut self: Box<Self>, addr: SocketAddr) -> Result<(), error::Error> {
        warp::serve(*self).run(addr).await;
        Ok(())
    }
}

#[cfg(feature = "web-warp")]
pub type ShuttleWarp<T> = Result<warp::filters::BoxedFilter<T>, Error>;
//pub type ShuttleWarp<T> = Result<T, Error>;

#[cfg(feature = "web-axum")]
#[async_trait]
impl Service for sync_wrapper::SyncWrapper<axum::Router> {
    async fn bind(mut self: Box<Self>, addr: SocketAddr) -> Result<(), error::Error> {
        let router = self.into_inner();

        axum::Server::bind(&addr)
            .serve(router.into_make_service())
            .await
            .map_err(error::CustomError::new)?;

        Ok(())
    }
}

#[cfg(feature = "web-axum")]
pub type ShuttleAxum = Result<sync_wrapper::SyncWrapper<axum::Router>, Error>;

#[cfg(feature = "web-tide")]
#[async_trait]
impl<T> Service for tide::Server<T>
where
    T: Clone + Send + Sync + 'static,
{
    async fn bind(mut self: Box<Self>, addr: SocketAddr) -> Result<(), error::Error> {
        self.listen(addr).await.map_err(error::CustomError::new)?;

        Ok(())
    }
}

#[cfg(feature = "web-tide")]
pub type ShuttleTide<T> = Result<tide::Server<T>, Error>;

#[cfg(feature = "web-tower")]
#[async_trait]
impl<T> Service for T
where
    T: tower::Service<hyper::Request<hyper::Body>, Response = hyper::Response<hyper::Body>>
        + Clone
        + Send
        + Sync
        + 'static,
    T::Error: std::error::Error + Send + Sync,
    T::Future: std::future::Future + Send + Sync,
{
    async fn bind(mut self: Box<Self>, addr: SocketAddr) -> Result<(), error::Error> {
        let shared = tower::make::Shared::new(self);
        hyper::Server::bind(&addr)
            .serve(shared)
            .await
            .map_err(error::CustomError::new)?;

        Ok(())
    }
}

#[cfg(feature = "bot-serenity")]
#[async_trait]
impl Service for serenity::Client {
    async fn bind(mut self: Box<Self>, _addr: SocketAddr) -> Result<(), error::Error> {
        self.start().await.map_err(error::CustomError::new)?;

        Ok(())
    }
}

#[cfg(feature = "bot-serenity")]
pub type ShuttleSerenity = Result<serenity::Client, Error>;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
