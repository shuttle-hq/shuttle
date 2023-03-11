#![doc(
    html_logo_url = "https://raw.githubusercontent.com/shuttle-hq/shuttle/main/assets/logo-square-transparent.png",
    html_favicon_url = "https://raw.githubusercontent.com/shuttle-hq/shuttle/main/assets/favicon.ico"
)]
//! # Shuttle - Deploy Rust apps with a single Cargo subcommand
//! <div style="display: flex; margin-top: 30px; margin-bottom: 30px;">
//! <img src="https://raw.githubusercontent.com/shuttle-hq/shuttle/main/assets/logo-rectangle-transparent.png" width="400px" style="margin-left: auto; margin-right: auto;"/>
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
//! Now that shuttle is installed, you can initialize a project with Rocket boilerplate:
//! ```bash
//! $ cargo shuttle init --rocket my-rocket-app
//! ```
//!
//! By looking at the `Cargo.toml` file of the generated `my-rocket-app` project you will see it has been made to
//! be a library crate with a `shuttle-service` dependency with the `web-rocket` feature on the `shuttle-service` dependency.
//!
//! ```toml
//! shuttle-service = { version = "0.8.0", features = ["web-rocket"] }
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
//! Or look at [more complete examples](https://github.com/shuttle-hq/examples), but
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
//! Before you can deploy, you have to create a project. This will start a deployer container for your
//! project under the hood, ensuring isolation from other users' projects.
//!
//! ```bash
//! $ cargo shuttle project new
//! ```
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
//! Add `shuttle-shared-db` as a dependency with the `postgres` feature, and add `sqlx` as a dependency with the `runtime-tokio-native-tls` and `postgres` features inside `Cargo.toml`:
//!
//! ```toml
//! shuttle-shared-db = { version = "0.8.0", features = ["postgres"] }
//! sqlx = { version = "0.6.2", features = ["runtime-tokio-native-tls", "postgres"] }
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
//! async fn rocket(#[shuttle_shared_db::Postgres] pool: PgPool) -> ShuttleRocket {
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
//! To learn more about shuttle managed resources, see [shuttle_service::main][main#getting-shuttle-managed-resources].
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
//! Alternatively, you can override the project name on the command-line, by passing the --name argument to any subcommand like so:
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
//! To stay updated with the release status of shuttle, [join our Discord](https://discord.gg/shuttle)!
//!
//! ## Join Discord
//!
//! If you have any questions, [join our Discord server](https://discord.gg/shuttle). There's always someone on there that can help!
//!
//! You can also [open an issue or a discussion on GitHub](https://github.com/shuttle-hq/shuttle).
//!

use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::path::PathBuf;

use async_trait::async_trait;

pub mod error;
pub use error::{CustomError, Error};

pub use shuttle_common::database;

#[cfg(feature = "codegen")]
extern crate shuttle_codegen;
#[cfg(feature = "codegen")]
/// Helper macro that generates the entrypoint required by any service - likely the only macro you need in this crate.
///
/// # Without shuttle managed resources
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
/// | Return type                           | Feature flag | Service                                     | Version    | Example                                                                               |
/// | ------------------------------------- | ------------ | ------------------------------------------- | ---------- | -----------------------------------------------------------------------------------   |
/// | `ShuttleRocket`                       | web-rocket   | [rocket](https://docs.rs/rocket/0.5.0-rc.2) | 0.5.0-rc.2 | [GitHub](https://github.com/shuttle-hq/examples/tree/main/rocket/hello-world)         |
/// | `ShuttleAxum`                         | web-axum     | [axum](https://docs.rs/axum/0.5)            | 0.5        | [GitHub](https://github.com/shuttle-hq/examples/tree/main/axum/hello-world)           |
/// | `ShuttleSalvo`                        | web-salvo    | [salvo](https://docs.rs/salvo/0.34.3)       | 0.34.3     | [GitHub](https://github.com/shuttle-hq/examples/tree/main/salvo/hello-world)          |
/// | `ShuttleTide`                         | web-tide     | [tide](https://docs.rs/tide/0.16.0)         | 0.16.0     | [GitHub](https://github.com/shuttle-hq/examples/tree/main/tide/hello-world)           |
/// | `ShuttlePoem`                         | web-poem     | [poem](https://docs.rs/poem/1.3.35)         | 1.3.35     | [GitHub](https://github.com/shuttle-hq/examples/tree/main/poem/hello-world)           |
/// | `Result<T, shuttle_service::Error>`   | web-tower    | [tower](https://docs.rs/tower/0.4.12)       | 0.14.12    | [GitHub](https://github.com/shuttle-hq/examples/tree/main/tower/hello-world)          |
/// | `ShuttleSerenity`                     | bot-serenity | [serenity](https://docs.rs/serenity/0.11.5) | 0.11.5     | [GitHub](https://github.com/shuttle-hq/examples/tree/main/serenity/hello-world)       |
/// | `ShuttlePoise`                        | bot-poise    | [poise](https://docs.rs/poise/0.5.2)        | 0.5.2      | [GitHub](https://github.com/shuttle-hq/examples/tree/main/poise/hello-world)          |
/// | `ShuttleActixWeb`                     | web-actix-web| [actix-web](https://docs.rs/actix-web/4.2.1)| 4.2.1      | [GitHub](https://github.com/shuttle-hq/examples/tree/main/actix-web/hello-world)      |
///
/// # Getting shuttle managed resources
/// Shuttle is able to manage resource dependencies for you. These resources are passed in as inputs to your `#[shuttle_service::main]` function and are configured using attributes:
/// ```rust,no_run
/// use sqlx::PgPool;
/// use shuttle_service::ShuttleRocket;
///
/// struct MyState(PgPool);
///
/// #[shuttle_service::main]
/// async fn rocket(#[shuttle_shared_db::Postgres] pool: PgPool) -> ShuttleRocket {
///     let state = MyState(pool);
///     let rocket = rocket::build().manage(state);
///
///     Ok(rocket)
/// }
/// ```
///
/// More [shuttle managed resources can be found here](https://github.com/shuttle-hq/shuttle/tree/main/resources)
pub use shuttle_codegen::main;

#[cfg(feature = "builder")]
pub mod builder;

pub use shuttle_common::project::ProjectName as ServiceName;

/// Factories can be used to request the provisioning of additional resources (like databases).
///
/// An instance of factory is passed by the deployer as an argument to [ResourceBuilder::build][ResourceBuilder::build] in the initial phase of deployment.
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

    /// Get all the secrets for a service
    async fn get_secrets(&mut self) -> Result<BTreeMap<String, String>, crate::Error>;

    /// Get the name for the service being deployed
    fn get_service_name(&self) -> ServiceName;

    /// Get the path where the build files are stored for this service
    fn get_build_path(&self) -> Result<PathBuf, crate::Error>;

    /// Get the path where files can be stored for this deployment
    fn get_storage_path(&self) -> Result<PathBuf, crate::Error>;
}

/// Used to get resources of type `T` from factories.
///
/// This is mainly meant for consumption by our code generator and should generally not be called by users.
///
/// ## Creating your own managed resource
/// You may want to create your own managed resource by implementing this trait for some builder `B` to construct resource `T`. [`Factory`] can be used to provision resources
/// on shuttle's servers if your resource will need any.
///
/// Your resource will be available on a [shuttle_service::main][main] function as follow:
/// ```
/// #[shuttle_service::main]
/// async fn my_service([custom_resource_crate::namespace::B] custom_resource: T)
///     -> shuttle_service::ShuttleAxum {}
/// ```
///
/// Here `custom_resource_crate::namespace` is the crate and namespace to a builder `B` that implements [`ResourceBuilder`] to create resource `T`.
///
/// ### Example
/// ```
/// pub struct Builder {
///     name: String,
/// }
///
/// pub struct Resource {
///     name: String,
/// }
///
/// impl Builder {
///     /// Name to give resource
///     pub fn name(self, name: &str) -> Self {
///         self.name = name.to_string();
///
///         self
///     }
/// }
///
/// #[async_trait]
/// impl ResourceBuilder<Resource> for Builder {
///     fn new() -> Self {
///         Self {
///             name: String::new(),
///         }
///     }
///
///     async fn build(
///         self,
///         factory: &mut dyn Factory,
///     ) -> Result<Resource, shuttle_service::Error> {
///         Ok(Resource { name: self.name })
///     }
/// }
/// ```
///
/// Then using this resource in a service:
/// ```
/// #[shuttle_service::main]
/// async fn my_service(
///     [custom_resource_crate::Builder(name = "John")] resource: custom_resource_crate::Resource
/// )
///     -> shuttle_service::ShuttleAxum {}
/// ```
#[async_trait]
pub trait ResourceBuilder<T> {
    fn new() -> Self;
    async fn build(self, factory: &mut dyn Factory) -> Result<T, crate::Error>;
}

/// The core trait of the shuttle platform. Every crate deployed to shuttle needs to implement this trait.
///
/// Use the [main][main] macro to expose your implementation to the deployment backend.
#[async_trait]
pub trait Service: Send {
    /// This function is run exactly once on each instance of a deployment.
    ///
    /// The deployer expects this instance of [Service][Service] to bind to the passed [SocketAddr][SocketAddr].
    async fn bind(mut self, addr: SocketAddr) -> Result<(), error::Error>;
}

#[cfg(feature = "web-rocket")]
#[async_trait]
impl Service for rocket::Rocket<rocket::Build> {
    async fn bind(mut self, addr: SocketAddr) -> Result<(), error::Error> {
        let shutdown = rocket::config::Shutdown {
            ctrlc: false,
            ..rocket::config::Shutdown::default()
        };

        let config = self
            .figment()
            .clone()
            .merge((rocket::Config::ADDRESS, addr.ip()))
            .merge((rocket::Config::PORT, addr.port()))
            .merge((rocket::Config::LOG_LEVEL, rocket::config::LogLevel::Off))
            .merge((rocket::Config::SHUTDOWN, shutdown));

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

#[cfg(feature = "web-warp")]
#[async_trait]
impl<T> Service for T
where
    T: Send + Sync + Clone + 'static + warp::Filter,
    T::Extract: warp::reply::Reply,
{
    async fn bind(mut self, addr: SocketAddr) -> Result<(), error::Error> {
        warp::serve(*self).run(addr).await;
        Ok(())
    }
}

#[cfg(feature = "web-warp")]
pub type ShuttleWarp<T> = Result<warp::filters::BoxedFilter<T>, Error>;

#[cfg(feature = "web-salvo")]
#[async_trait]
impl Service for salvo::Router {
    async fn bind(mut self, addr: SocketAddr) -> Result<(), error::Error> {
        salvo::Server::new(salvo::listener::TcpListener::bind(addr))
            .serve(self)
            .await;

        Ok(())
    }
}

#[cfg(feature = "web-salvo")]
pub type ShuttleSalvo = Result<salvo::Router, Error>;

#[cfg(feature = "web-thruster")]
#[async_trait]
impl<T> Service for T
where
    T: thruster::ThrusterServer + Sync + Send + 'static,
{
    async fn bind(mut self, addr: SocketAddr) -> Result<(), error::Error> {
        self.build(&addr.ip().to_string(), addr.port()).await;

        Ok(())
    }
}

#[cfg(feature = "web-thruster")]
pub type ShuttleThruster<T> = Result<T, Error>;

#[cfg(feature = "web-tide")]
#[async_trait]
impl<T> Service for tide::Server<T>
where
    T: Clone + Send + Sync + 'static,
{
    async fn bind(mut self, addr: SocketAddr) -> Result<(), error::Error> {
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
    async fn bind(mut self, addr: SocketAddr) -> Result<(), error::Error> {
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
    async fn bind(mut self, _addr: SocketAddr) -> Result<(), error::Error> {
        self.start().await.map_err(error::CustomError::new)?;

        Ok(())
    }
}

#[cfg(feature = "bot-serenity")]
pub type ShuttleSerenity = Result<serenity::Client, Error>;

pub const NEXT_NAME: &str = "shuttle-next";
