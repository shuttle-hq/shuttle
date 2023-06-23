use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::path::PathBuf;

use async_trait::async_trait;
use serde::{de::DeserializeOwned, Serialize};
pub use shuttle_common::{
    database, deployment::Environment, project::ProjectName as ServiceName, resource::Type,
    DatabaseReadyInfo, DbInput, DbOutput, SecretStore,
};

pub mod error;
pub use error::{CustomError, Error};

#[cfg(feature = "builder")]
pub mod builder;

pub const NEXT_NAME: &str = "shuttle-next";
pub const RUNTIME_NAME: &str = "shuttle-runtime";

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
/// async fn my_service(
///     [custom_resource_crate::namespace::B] custom_resource: T,
/// ) -> shuttle_axum::ShuttleAxum {}
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
///     const TYPE: Type = Type::Custom;
///
///     type Config = Self;
///
///     type Output = String;
///
///     fn new() -> Self {
///         Self {
///             name: String::new(),
///         }
///     }
///
///     fn config(&self) -> &Self::Config {
///         &self
///     }
///
///     async fn output(self, factory: &mut dyn Factory) -> Result<Self::Output, shuttle_service::Error> {
///         Ok(self.name)
///     }
///
///     async fn build(build_data: &Self::Output) -> Result<Resource, shuttle_service::Error> {
///         Ok(Resource { name: build_data })
///     }
/// }
/// ```
///
/// Then using this resource in a service:
/// ```
/// #[shuttle_runtime::main]
/// async fn my_service(
///     [custom_resource_crate::Builder(name = "John")] resource: custom_resource_crate::Resource
/// ) -> shuttle_axum::ShuttleAxum {}
/// ```
#[async_trait]
pub trait ResourceBuilder<T> {
    /// The type of resource this creates
    const TYPE: Type;

    /// The internal config being constructed by this builder. This will be used to find cached [Self::Output].
    type Config: Serialize;

    /// The output type used to build this resource later
    type Output: Serialize + DeserializeOwned;

    /// Create a new instance of this resource builder
    fn new() -> Self;

    /// Get the internal config state of the builder
    ///
    /// If the exact same config was returned by a previous deployement that used this resource, then [Self::output()]
    /// will not be called to get the builder output again. Rather the output state of the previous deployment
    /// will be passed to [Self::build()].
    fn config(&self) -> &Self::Config;

    /// Get the config output of this builder
    ///
    /// This method is where the actual resource provisioning should take place and is expected to take the longest. It
    /// can at times even take minutes. That is why the output of this method is cached and calling this method can be
    /// skipped as explained in [Self::config()].
    async fn output(self, factory: &mut dyn Factory) -> Result<Self::Output, crate::Error>;

    /// Build this resource from its config output
    async fn build(build_data: &Self::Output) -> Result<T, crate::Error>;
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
