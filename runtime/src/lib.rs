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
mod alpha;
mod logger;
#[cfg(feature = "next")]
mod next;
mod provisioner_factory;
mod resource_tracker;

pub use alpha::{start, Alpha};
pub use async_trait::async_trait;
pub use logger::Logger;
#[cfg(feature = "next")]
pub use next::{AxumWasm, NextArgs};
pub use provisioner_factory::ProvisionerFactory;
pub use resource_tracker::{get_resource, ResourceTracker};
pub use shuttle_common::storage_manager::StorageManager;
pub use shuttle_service::{main, CustomError, Error, Factory, ResourceBuilder, Service};

// Dependencies required by the codegen
pub use anyhow::Context;
pub use strfmt::strfmt;
pub use tracing;
pub use tracing_subscriber;
