use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::path::PathBuf;

use async_trait::async_trait;

pub mod error;
pub use error::{CustomError, Error};

use serde::{de::DeserializeOwned, Serialize};
use shuttle_common::DatabaseReadyInfo;
pub use shuttle_common::{database, resource::Type};

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
/// An instance of factory is passed by the deployer as an argument to [ResourceBuilder::build][ResourceBuilder::output] in the initial phase of deployment.
///
/// Also see the [main][main] macro.
#[async_trait]
pub trait Factory: Send + Sync {
    /// Get a database connection
    async fn get_db_connection(
        &mut self,
        db_type: database::Type,
    ) -> Result<DatabaseReadyInfo, crate::Error>;

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
pub trait ResourceBuilder<T>: Serialize {
    /// The type of resource this creates
    const TYPE: Type;

    /// The output type used to build this resource later
    type Output: Serialize + DeserializeOwned;

    /// Create a new instance of this resource builder
    fn new() -> Self;

    /// Build this resource from its config output
    async fn build(build_data: &Self::Output) -> Result<T, crate::Error>;

    /// Get the config output of this builder
    async fn output(self, factory: &mut dyn Factory) -> Result<Self::Output, crate::Error>;
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
