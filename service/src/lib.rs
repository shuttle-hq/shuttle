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

/// An interface for the provisioner used in [`ResourceBuilder::output`].
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

/// Allows implementing plugins for the Shuttle main function.
#[async_trait]
pub trait ResourceBuilder {
    /// The type of resource this plugin creates.
    const TYPE: resource::Type;

    /// The input config to this resource.
    type Config: Default + Serialize;

    /// The output from requesting this resource.
    /// A cached copy of this will be used if the same [`ResourceBuilder::Config`] is found for this [`ResourceBuilder::TYPE`].
    type Output: Serialize + DeserializeOwned;

    /// Get the config of this plugin after it has been built from its macro arguments with the builder pattern.
    ///
    /// If the exact same config was returned by a previous deployment that used this resource, then [`ResourceBuilder::output`]
    /// will not be called to get the builder output again. Rather the output state of the previous deployment
    /// will be passed to [`ResourceBuilder::build`].
    fn config(&self) -> &Self::Config;

    /// Construct this resource with the help of metadata and by calling provisioner methods in the [`Factory`].
    ///
    /// This method is where the actual resource provisioning should take place and is expected to take the longest. It
    /// can at times even take minutes. That is why the output of this method is cached and calling this method can be
    /// skipped as explained in [`ResourceBuilder::config`].
    ///
    /// The output from this function is passed to [`IntoResource::init`].
    async fn output(self, factory: &mut dyn Factory) -> Result<Self::Output, crate::Error>;
}

/// Implement this on an [`ResourceBuilder::Output`] type to turn the
/// base resource into the end type exposed to the Shuttle main function.
#[async_trait]
pub trait IntoResource<R>: Serialize + DeserializeOwned {
    async fn init(self) -> Result<R, crate::Error>;
}

/// The core trait of the Shuttle platform. Every service deployed to Shuttle needs to implement this trait.
///
/// Use the [`shuttle_runtime::main`] macro to expose your implementation to the deployment backend.
#[async_trait]
pub trait Service: Send {
    /// This function is run exactly once on each instance of a deployment.
    ///
    /// The deployer expects this instance of [`Service`] to bind to the passed [`SocketAddr`].
    async fn bind(mut self, addr: SocketAddr) -> Result<(), error::Error>;
}
