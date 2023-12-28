use std::collections::BTreeMap;
use std::net::SocketAddr;

use async_trait::async_trait;
use serde::{de::DeserializeOwned, Serialize};
pub use shuttle_common::secrets::Secret;
pub use shuttle_common::{
    database,
    deployment::{DeploymentMetadata, Environment},
    resource::Type,
    DatabaseReadyInfo, DbInput, DbOutput, QdrantInput, QdrantReadyInfo, SecretStore,
};

pub use crate::error::{CustomError, Error};

#[cfg(feature = "builder")]
pub mod builder;
pub mod error;
#[cfg(feature = "runner")]
pub mod runner;

/// Factories can be used to request the provisioning of additional resources (like databases).
///
/// An instance of factory is passed by the deployer as an argument to [ResourceBuilder::output] in the initial phase of deployment.
///
/// Also see the [shuttle_runtime::main] macro.
#[async_trait]
pub trait Factory: Send + Sync {
    /// Get a database connection
    async fn get_db_connection(
        &mut self,
        db_type: database::Type,
    ) -> Result<DatabaseReadyInfo, crate::Error>;

    /// Get a Qdrant connection. Only used in local runs.
    async fn get_qdrant_connection(
        &mut self,
        project_name: String,
    ) -> Result<QdrantReadyInfo, crate::Error>;

    /// Get all the secrets for a service
    async fn get_secrets(&mut self) -> Result<BTreeMap<String, Secret<String>>, crate::Error>;

    /// Get the metadata for this deployment
    fn get_metadata(&self) -> DeploymentMetadata;
}

/// Used to get resources of type `T` from factories.
///
/// This is mainly meant for consumption by our code generator and should generally not be called by users.
///
/// ## Creating your own managed resource
///
/// You may want to create your own managed resource by implementing this trait for some builder `B` to construct resource `T`.
/// [`Factory`] can be used to provision resources on Shuttle's servers if your service will need any.
///
/// Please refer to `shuttle-examples/custom-resource` for examples of how to create custom resource. For more advanced provisioning
/// of custom resources, please [get in touch](https://discord.gg/shuttle) and detail your use case. We'll be interested to see what you
/// want to provision and how to do it on your behalf on the fly.
///
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
    /// If the exact same config was returned by a previous deployment that used this resource, then [Self::output()]
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
