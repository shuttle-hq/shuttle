use std::collections::BTreeMap;
use std::net::SocketAddr;

use async_trait::async_trait;
use serde::{de::DeserializeOwned, Serialize};
pub use shuttle_common::{
    database,
    deployment::{DeploymentMetadata, Environment},
    resource,
    secrets::Secret,
    DatabaseInfo, DatabaseResource, DbInput, SecretStore,
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
    ) -> Result<DatabaseInfo, crate::Error>;

    /// Get all the secrets for a service
    async fn get_secrets(&mut self) -> Result<BTreeMap<String, Secret<String>>, crate::Error>;

    /// Get the metadata for this deployment
    fn get_metadata(&self) -> DeploymentMetadata;
}

/// Used to get resources of type `Output` from factories.
///
/// This is mainly meant for consumption by our code generator and should generally not be called by users.
///
/// TODO: New docs
#[async_trait]
pub trait ResourceBuilder {
    /// The type of resource this creates
    const TYPE: resource::Type;

    /// The input config to this resource.
    type Config: Default + Serialize;

    /// The output from requesting this resource.
    /// A cached copy of this will be used if the same [`Self::Config`] is found for this [`Self::TYPE`].
    type Output: Serialize + DeserializeOwned;

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
}

/// Implement this on an ResourceBuilder::Output type to turn the
/// base resource into the end type exposed to the shuttle main function.
#[async_trait]
pub trait IntoResource<R>: Serialize + DeserializeOwned {
    async fn init(self) -> Result<R, crate::Error>;
}

// #[async_trait]
// impl<T1, T2> IntoResource<T1> for T2 {
//     type Output = T2;
//     async fn init(r: T1) -> Result<Self::Output, crate::Error> {
//         r.init().await
//     }
// }

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
