use async_trait::async_trait;
use serde::Serialize;
use shuttle_service::{
    Factory, ResourceBuilder, Type, DynamoDBInput
};
pub use shuttle_service::DynamoDbReadyInfo;

#[derive(Serialize)]
#[doc = "A resource connected to an AWS DynamoDB  instance"]
pub struct DynamoDB {
    config: DynamoDBInput
}

#[doc = "Gets a connection to DynamoDB"]
#[async_trait]
impl ResourceBuilder<DynamoDbReadyInfo> for DynamoDB {
    const TYPE: Type = Type::DynamoDB;
    
    type Config = DynamoDBInput; 
    type Output = DynamoDbReadyInfo;

    fn new() -> Self {
        Self { config: Default::default() }
    }

    fn config(&self) -> &Self::Config {
        &self.config
    }

    async fn output(self, factory: &mut dyn Factory) -> Result<Self::Output, shuttle_service::Error> {
        factory
            .get_dynamodb_connection()
            .await
    }

    async fn build(build_data: &Self::Output) -> Result<DynamoDbReadyInfo, shuttle_service::Error> {
        Ok(build_data.clone())
    }
}