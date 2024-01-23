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
pub use shuttle_proto::provisioner::{ContainerRequest, ContainerResponse};

pub use crate::error::{CustomError, Error};

#[cfg(feature = "builder")]
pub mod builder;
pub mod error;
#[cfg(feature = "runner")]
pub mod runner;

/// An interface for the provisioner used in [`ResourceBuilder::output`].
#[async_trait]
pub trait Factory: Send + Sync {
    /// Provision a Shuttle database and get the connection information
    async fn get_db_connection(
        &mut self,
        db_type: database::Type,
    ) -> Result<DatabaseInfo, crate::Error>;

    /// Start a Docker container. Only used in local runs.
    async fn get_container(
        &mut self,
        req: ContainerRequest,
    ) -> Result<ContainerResponse, crate::Error>;

    /// Get the secrets associated with this service
    async fn get_secrets(&mut self) -> Result<BTreeMap<String, Secret<String>>, crate::Error>;

    /// Get the metadata for this deployment
    fn get_metadata(&self) -> DeploymentMetadata;
}

/// Allows implementing plugins for the Shuttle main function.
///
/// ## Creating your own Shuttle plugin
///
/// You can add your own implementation of this trait along with [`IntoResource<R>`] to customize the
/// input type `R` that gets into the Shuttle main function on an existing resource.
/// The [`Factory`] in [`ResourceBuilder::output`] can be used to provision resources on Shuttle's servers if your service will need any.
///
/// You can also make your own plugin, for example to generalise the connection logic to a third-party service.
/// One example of this is `shuttle-qdrant`.
///
/// Please refer to `shuttle-examples/custom-resource` for examples of how to create a custom resource. For more advanced provisioning
/// of custom resources, please [get in touch](https://discord.gg/shuttle) and detail your use case. We'll be interested to see what you
/// want to provision and how to do it on your behalf on the fly.
#[async_trait]
pub trait ResourceBuilder: Default {
    /// The type of resource this plugin creates.
    /// If dealing with a Shuttle-provisioned resource, such as a database, use the corresponding variant.
    /// Otherwise, use the `Custom` variant.
    const TYPE: resource::Type;

    /// The input config to this resource.
    type Config: Serialize;

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
    /// The output from this function is passed to [`IntoResource::into_resource`].
    async fn output(self, factory: &mut dyn Factory) -> Result<Self::Output, crate::Error>;
}

/// Implement this on an [`ResourceBuilder::Output`] type to turn the
/// base resource into the end type exposed to the Shuttle main function.
#[async_trait]
pub trait IntoResource<R>: Serialize + DeserializeOwned {
    /// Initialize any logic for creating the final resource of type `R` from the base resource.
    ///
    /// Example: turn a connection string into a connection pool.
    async fn into_resource(self) -> Result<R, crate::Error>;
}

// Base impl for [`ResourceBuilder::Output`] types that don't need to convert into anything else
#[async_trait]
impl<R: Serialize + DeserializeOwned + Send> IntoResource<R> for R {
    async fn into_resource(self) -> Result<R, crate::Error> {
        Ok(self)
    }
}

/// The core trait of the Shuttle platform. Every service deployed to Shuttle needs to implement this trait.
///
/// An `Into<Service>` implementor is what is returned in the [`shuttle_runtime::main`] macro
/// in order to run it on the Shuttle servers.
#[async_trait]
pub trait Service: Send {
    /// This function is run exactly once on startup of a deployment.
    ///
    /// The passed [`SocketAddr`] receives proxied HTTP traffic from you Shuttle subdomain (or custom domain).
    /// Binding to the address is only relevant if this service is an HTTP server.
    async fn bind(mut self, addr: SocketAddr) -> Result<(), error::Error>;
}
