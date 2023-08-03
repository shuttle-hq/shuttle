use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use shuttle_service::{error::Error, Factory, ResourceBuilder, Type};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ServiceInfo {
    /// The Shuttle service name.
    name: String,
}

impl ServiceInfo {
    /// Get the Shuttle service name.
    pub fn name(&self) -> &str {
        &self.name
    }
}

pub struct ShuttleServiceInfo;

#[async_trait]
impl ResourceBuilder<ServiceInfo> for ShuttleServiceInfo {
    fn new() -> Self {
        Self
    }

    const TYPE: Type = Type::ServiceInfo;

    type Config = ();

    type Output = ServiceInfo;

    fn config(&self) -> &Self::Config {
        &()
    }

    async fn output(self, factory: &mut dyn Factory) -> Result<Self::Output, Error> {
        Ok(ServiceInfo {
            name: factory.get_service_name().to_string(),
        })
    }

    async fn build(build_data: &Self::Output) -> Result<ServiceInfo, Error> {
        Ok(build_data.clone())
    }
}
