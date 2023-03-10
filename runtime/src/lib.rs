mod legacy;
mod logger;
#[cfg(feature = "next")]
mod next;
mod provisioner_factory;

pub use async_trait::async_trait;
pub use legacy::{start, Legacy};
pub use logger::Logger;
#[cfg(feature = "next")]
pub use next::{AxumWasm, NextArgs};
pub use provisioner_factory::ProvisionerFactory;
pub use shuttle_common::storage_manager::StorageManager;
pub use shuttle_service::{main, Error, ResourceBuilder, Service};

pub type CustomError = anyhow::Error;
pub use anyhow::Context;
pub use tracing;
pub use tracing_subscriber;
