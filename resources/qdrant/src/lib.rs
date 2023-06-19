use async_trait::async_trait;
use serde::Serialize;
pub use shuttle_service::QdrantReadyInfo;
use shuttle_service::{Factory, QdrantInput, ResourceBuilder, Type};

#[derive(Serialize)]
#[doc = "A resource connected to a Qdrant instance"]
pub struct Qdrant {
    config: QdrantInput
}

#[doc = "Gets a connection to Qdrant"]
#[async_trait]
impl ResourceBuilder<QdrantReadyInfo> for Qdrant {
    const TYPE: Type = Type::Qdrant;

    type Config = QdrantInput;
    type Output = QdrantReadyInfo;

    fn new() -> Self {
        Self {
            config: Default::default(),
        }
    }

    fn config(&self) -> &Self::Config {
        &self.config
    }

    async fn output(
        self,
        factory: &mut dyn Factory,
    ) -> Result<Self::Output, shuttle_service::Error> {
        factory.get_qdrant_connection().await
    }

    async fn build(build_data: &Self::Output) -> Result<QdrantReadyInfo, shuttle_service::Error> {
        Ok(build_data.clone())
    }
}
