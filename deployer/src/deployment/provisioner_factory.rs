use std::collections::BTreeMap;
use std::str::FromStr;

use async_trait::async_trait;
use shuttle_common::database;
use shuttle_service::{Factory, ServiceName};

/// Trait to make it easy to get a factory (service locator) for each service being started
pub trait AbstractFactory: Send + 'static {
    type Output: Factory;

    /// Get a factory for a specific service
    fn get_factory(&self) -> Self::Output;
}

/// An abstract factory that makes factories which uses provisioner
#[derive(Clone)]
pub struct AbstractDummyFactory;

impl AbstractFactory for AbstractDummyFactory {
    type Output = DummyFactory;

    fn get_factory(&self) -> Self::Output {
        DummyFactory::new()
    }
}

impl AbstractDummyFactory {
    pub fn new() -> Self {
        Self
    }
}

pub struct DummyFactory {
    service_name: ServiceName,
}

impl DummyFactory {
    fn new() -> Self {
        Self {
            service_name: ServiceName::from_str("legacy").unwrap(),
        }
    }
}

#[async_trait]
impl Factory for DummyFactory {
    fn get_service_name(&self) -> ServiceName {
        self.service_name.clone()
    }

    async fn get_db_connection_string(
        &mut self,
        _: database::Type,
    ) -> Result<String, shuttle_service::Error> {
        todo!()
    }

    async fn get_secrets(&mut self) -> Result<BTreeMap<String, String>, shuttle_service::Error> {
        todo!()
    }
}
