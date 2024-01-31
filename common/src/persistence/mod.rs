use thiserror::Error;
use ulid::Ulid;
use uuid::Uuid;

use self::deployment::{
    ActiveDeploymentsGetter, AddressGetter, Deployment, DeploymentRunnable, DeploymentUpdater,
};
use self::service::Service;

pub mod deployment;
pub mod service;
pub mod state;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Hyper error: {0}")]
    Sqlx(#[from] sqlx::Error),
    #[error("Failed to convert {from} to {to}")]
    Convert {
        from: String,
        to: String,
        message: String,
    },
}

#[async_trait::async_trait]
pub trait DeployerPersistenceApi:
    DeploymentUpdater + ActiveDeploymentsGetter + AddressGetter
{
    type MasterErr: std::error::Error + Send;

    async fn insert_deployment(
        &self,
        deployment: impl Into<&Deployment> + Send,
    ) -> Result<(), Self::MasterErr>;

    async fn get_deployment(&self, id: &Uuid) -> Result<Option<Deployment>, Self::MasterErr>;

    async fn get_deployments(
        &self,
        service_id: &Ulid,
        offset: u32,
        limit: u32,
    ) -> Result<Vec<Deployment>, Self::MasterErr>;

    async fn get_active_deployment(
        &self,
        service_id: &Ulid,
    ) -> Result<Option<Deployment>, Self::MasterErr>;

    async fn cleanup_invalid_states(&self) -> Result<(), Self::MasterErr>;

    async fn get_service_by_name(&self, name: &str) -> Result<Option<Service>, Self::MasterErr>;

    async fn get_or_create_service(&self, name: &str) -> Result<Service, Self::MasterErr>;

    async fn delete_service(&self, id: &Ulid) -> Result<(), Self::MasterErr>;

    async fn get_all_services(&self) -> Result<Vec<Service>, Self::MasterErr>;

    async fn get_all_runnable_deployments(
        &self,
    ) -> Result<Vec<DeploymentRunnable>, Self::MasterErr>;

    async fn get_runnable_deployment(
        &self,
        id: &Uuid,
    ) -> Result<Option<DeploymentRunnable>, Self::MasterErr>;
}
