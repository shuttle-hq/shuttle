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
//! Now that shuttle is installed, you can initialize a project with Axum boilerplate:
//! ```bash
//! $ cargo shuttle init --axum my-axum-app
//! ```
//!
//! By looking at the `Cargo.toml` file of the generated `my-axum-app` project you will see it has been made to
//! be a binary crate with a few dependencies including `shuttle-runtime` and `shuttle-axum`.
//!
//! ```toml
//! shuttle-runtime = "0.12.0"
//! axum = "0.6.10"
//! shuttle-axum = "0.12.0"
//! tokio = "1.26"
//! ```
//!
//! A boilerplate code for your axum project can also be found in `src/main.rs`:
//!
//! ```rust,no_run
//! use axum::{routing::get, Router};
//!
//! async fn hello_world() -> &'static str {
//!     "Hello, world!"
//! }
//!
//! #[shuttle_runtime::main]
//! async fn axum() -> shuttle_axum::ShuttleAxum {
//!     let router = Router::new().route("/hello", get(hello_world));
//!
//!     Ok(router.into())
//! }
//! ```
//!
//! Check out [our docs](https://docs.shuttle.rs/introduction/welcome) to see all the frameworks we support, or
//! our [examples](https://github.com/shuttle-hq/examples) if you prefer that format.
//!
//! ## Running locally
//! To test your app locally before deploying, use:
//!
//! ```bash
//! $ cargo shuttle run
//! ```
//!
//! You should see your app build and start on the default port 8000. You can test this using;
//!
//! ```bash
//! $ curl http://localhost:8000/hello
//!
//! Hello, world!
//! ```
//!
//! ## Deploying
//!
//! You can deploy your service with the [`cargo shuttle`](https://docs.rs/crate/cargo-shuttle/latest) subcommand too.
//! But, you will need to authenticate with the shuttle service first using:
//!
//! ```bash
//! $ cargo shuttle login
//! ```
//!
//! This will open a browser window and prompt you to connect using your GitHub account.
//!
//! Before you can deploy, you have to create a project. This will start a deployer container for your
//! project under the hood, ensuring isolation from other users' projects. PS. you don't have to do this
//! now if you did in in the `cargo shuttle init` flow.
//!
//! ```bash
//! $ cargo shuttle project new
//! ```
//!
//! Then, deploy the service with:
//!
//! ```bash
//! $ cargo shuttle deploy
//! ```
//!
//! Your service will immediately be available at `{crate_name}.shuttleapp.rs`. For example:
//!
//! ```bash
//! $ curl https://my-axum-app.shuttleapp.rs/hello
//! Hello, world!
//! ```
//!
//! ## Using `sqlx`
//!
//! Here is a quick example to deploy a rocket service that uses a postgres database and [sqlx](http://docs.rs/sqlx):
//!
//! Initialize a project with Rocket boilerplate:
//! ```bash
//! $ cargo shuttle init --rocket my-rocket-app
//! ```
//!
//! Add `shuttle-shared-db` as a dependency with the `postgres` feature, and add `sqlx` as a dependency with the
//! `runtime-tokio-native-tls` and `postgres` features inside `Cargo.toml`:
//!
//! ```toml
//! shuttle-shared-db = { version = "0.12.0", features = ["postgres"] }
//! sqlx = { version = "0.6.2", features = ["runtime-tokio-native-tls", "postgres"] }
//! ```
//!
//! Now update the `#[shuttle_runtime::main]` function to take in a `PgPool`:
//!
//! ```rust,no_run
//! #[macro_use]
//! extern crate rocket;
//!
//! use rocket::State;
//! use sqlx::PgPool;
//! use shuttle_rocket::ShuttleRocket;
//!
//! struct MyState(PgPool);
//!
//! #[get("/hello")]
//! fn hello(state: &State<MyState>) -> &'static str {
//!     // Do things with `state.0`...
//!     "Hello, Postgres!"
//! }
//!
//! #[shuttle_runtime::main]
//! async fn rocket(#[shuttle_shared_db::Postgres] pool: PgPool) -> ShuttleRocket {
//!     let state = MyState(pool);
//!     let rocket = rocket::build().manage(state).mount("/", routes![hello]);
//!
//!     Ok(rocket.into())
//! }
//! ```
//!
//! For a local run, shuttle will automatically provision a Postgres instance inside a [Docker](https://www.docker.com/) container on your machine and connect it to the `PgPool`.
//!
//! For deploys, shuttle will provision a database for your application and connect it to the `PgPool` on your behalf.
//!
//! To learn more about shuttle managed resources, see our [resource docs](https://docs.shuttle.rs/resources/shuttle-shared-db).
//!
//! ## Configuration
//!
//! The `cargo shuttle` command can be customized by creating a `Shuttle.toml` in the same location as your `Cargo.toml`.
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
//! $ cargo shuttle deploy --name=$PROJECT_NAME
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
/// use shuttle_rocket::ShuttleRocket;
///
/// #[shuttle_rocket::main]
/// async fn rocket() -> ShuttleRocket {
///     let rocket = rocket::build();
///
///     Ok(rocket.into())
/// }
/// ```
///
/// ## shuttle supported services
/// The following types can be returned from a `#[shuttle_service::main]` function and enjoy first class service support in shuttle.
///
/// | Return type                           | Crate                                                         | Service                                     | Version    | Example                                                                               |
/// | ------------------------------------- |-------------------------------------------------------------- | ------------------------------------------- | ---------- | -----------------------------------------------------------------------------------   |
/// | `ShuttleActixWeb`                     |[shuttle-actix-web](https://crates.io/crates/shuttle-actix-web)| [actix-web](https://docs.rs/actix-web/4.3)  | 4.3        | [GitHub](https://github.com/shuttle-hq/examples/tree/main/actix-web/hello-world)      |
/// | `ShuttleAxum`                         |[shuttle-axum](https://crates.io/crates/shuttle-axum)          | [axum](https://docs.rs/axum/0.6)            | 0.5        | [GitHub](https://github.com/shuttle-hq/examples/tree/main/axum/hello-world)           |
/// | `ShuttlePoem`                         |[shuttle-poem](https://crates.io/crates/shuttle-poem)          | [poem](https://docs.rs/poem/1.3)            | 1.3        | [GitHub](https://github.com/shuttle-hq/examples/tree/main/poem/hello-world)           |
/// | `ShuttlePoise`                        |[shuttle-poise](https://crates.io/crates/shuttle-poise)        | [poise](https://docs.rs/poise/0.5)          | 0.5        | [GitHub](https://github.com/shuttle-hq/examples/tree/main/poise/hello-world)          |
/// | `ShuttleRocket`                       |[shuttle-rocket](https://crates.io/crates/shuttle-rocket)      | [rocket](https://docs.rs/rocket/0.5.0-rc.2) | 0.5.0-rc.2 | [GitHub](https://github.com/shuttle-hq/examples/tree/main/rocket/hello-world)         |
/// | `ShuttleSalvo`                        |[shuttle-salvo](https://crates.io/crates/shuttle-salvo)        | [salvo](https://docs.rs/salvo/0.37)         | 0.37       | [GitHub](https://github.com/shuttle-hq/examples/tree/main/salvo/hello-world)          |
/// | `ShuttleSerenity`                     |[shuttle-serenity](https://crates.io/crates/shuttle-serenity   | [serenity](https://docs.rs/serenity/0.11)   | 0.11       | [GitHub](https://github.com/shuttle-hq/examples/tree/main/serenity/hello-world)       |
/// | `ShuttleThruster`                     |[shuttle-thruster](https://crates.io/crates/shuttle-thruster)  | [thruster](https://docs.rs/thruster/1.3)    | 1.3        | [GitHub](https://github.com/shuttle-hq/examples/tree/main/thruster/hello-world)       |
/// | `ShuttleTower`                        |[shuttle-tower](https://crates.io/crates/shuttle-tower)        | [tower](https://docs.rs/tower/0.4)          | 0.4        | [GitHub](https://github.com/shuttle-hq/examples/tree/main/tower/hello-world)          |
/// | `ShuttleTide`                         |[shuttle-tide](https://crates.io/crates/shuttle-tide)          | [tide](https://docs.rs/tide/0.16)           | 0.16       | [GitHub](https://github.com/shuttle-hq/examples/tree/main/tide/hello-world)           |
///
/// # Getting shuttle managed resources
/// Shuttle is able to manage resource dependencies for you. These resources are passed in as inputs to your `#[shuttle_runtime::main]` function and are configured using attributes:
/// ```rust,no_run
/// use sqlx::PgPool;
/// use shuttle_rocket::ShuttleRocket;
///
/// struct MyState(PgPool);
///
/// #[shuttle_runtime::main]
/// async fn rocket(#[shuttle_shared_db::Postgres] pool: PgPool) -> ShuttleRocket {
///     let state = MyState(pool);
///     let rocket = rocket::build().manage(state);
///
///     Ok(rocket.into())
/// }
/// ```
///
/// More [shuttle managed resources can be found here](https://github.com/shuttle-hq/shuttle/tree/main/resources)
pub use shuttle_codegen::main;

#[cfg(feature = "builder")]
pub mod builder;

pub use shuttle_common::{deployment::Environment, project::ProjectName as ServiceName};

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

    /// Get the environment for this deployment
    fn get_environment(&self) -> Environment;

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
/// Your resource will be available on a [shuttle_runtime::main][main] function as follow:
/// ```
/// #[shuttle_runtime::main]
/// async fn my_service([custom_resource_crate::namespace::B] custom_resource: T)
///     -> shuttle_axum::ShuttleAxum {}
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
/// #[shuttle_runtime::main]
/// async fn my_service(
///     [custom_resource_crate::Builder(name = "John")] resource: custom_resource_crate::Resource
/// )
///     -> shuttle_axum::ShuttleAxum {}
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

pub const NEXT_NAME: &str = "shuttle-next";
