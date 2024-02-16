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

/// An interface for the provisioner used in [`IntoResourceConfig::Config`].
pub trait Factory: Send + Sync {
    /// Get the secrets associated with this service
    fn get_secrets(&self) -> Result<BTreeMap<String, Secret<String>>, crate::Error>;

    /// Get the metadata for this deployment
    fn get_metadata(&self) -> DeploymentMetadata;
}

/// Allows implementing plugins for the Shuttle main function.
///
/// ## Creating your own Shuttle plugin
///
/// You can add your own implementation of this trait along with [`IntoResource`] to customize the
/// input type `R` that gets into the Shuttle main function on an existing resource.
///
/// You can also make your own plugin, for example to generalise the connection logic to a third-party service.
/// One example of this is `shuttle-qdrant`.
///
/// Please refer to `shuttle-examples/custom-resource` for examples of how to create a custom resource. For more advanced provisioning
/// of custom resources, please [get in touch](https://discord.gg/shuttle) and detail your use case. We'll be interested to see what you
/// want to provision and how to do it on your behalf on the fly.
#[async_trait]
pub trait IntoResourceInput: Default {
    /// The input for requesting this resource.
    ///
    /// If the input is a [`shuttle_common::resource::ProvisionResourceRequest`],
    /// then the resource will be provisioned and the [`IntoResourceInput::Output`]
    /// will be a [`shuttle_common::resource::ShuttleResourceOutput<T>`] with the resource's associated output type.
    type Input: Serialize + DeserializeOwned;

    /// The output from provisioning this resource.
    ///
    /// For custom resources that don't provision anything from Shuttle,
    /// this should be the same type as [`IntoResourceInput::Input`].
    ///
    /// This type must implement [`IntoResource`] for the desired final resource type `R`.
    type Output: Serialize + DeserializeOwned;

    /// Construct this resource config. The [`Factory`] provides access to secrets and metadata.
    async fn into_resource_input(self, factory: &dyn Factory) -> Result<Self::Input, crate::Error>;
}

/// Implement this on an [`IntoResourceInput::Output`] type to turn the
/// base resource into the end type exposed to the Shuttle main function.
#[async_trait]
pub trait IntoResource<R>: Serialize + DeserializeOwned {
    /// Initialize any logic for creating the final resource of type `R` from the base resource.
    ///
    /// Example: turn a connection string into a connection pool.
    async fn into_resource(self) -> Result<R, crate::Error>;
}

// Base impl for [`IntoResourceInput::Output`] types that don't need to convert into anything else
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
