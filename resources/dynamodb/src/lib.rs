use async_trait::async_trait;
use serde::Serialize;
use shuttle_service::{
    database, error::CustomError, DbInput, DbOutput, Error, Factory, ResourceBuilder, Type, DynamoDBInput, DynamoDBOutput
};

#[derive(Serialize)]
#[doc = "A resource connected to an AWS DynamoDB  instance"]
pub struct DynamoDB {
    config: DynamoDBInput
}

#[doc = "Gets a connection to DynamoDB"]
#[async_trait]
impl<T> ResourceBuilder<T> for DynamoDB {
    const TYPE: Type = Type::DynamoDB;
    
    // These may change later
    type Config = DynamoDBInput; 
    type Output = DynamoDBOutput;

    fn new() -> Self {
        Self { config: Default::default() }
    }

    fn config(&self) -> &Self::Config {
        &self.config
    }

    async fn output(self, factory: &mut dyn Factory) -> Result<Self::Output, shuttle_service::Error> {
        
        let info = match factory.get_environment() {
            shuttle_service::Environment::Production => DynamoDBOutput::Info(
                factory
                    .get_dynamodb_connection()
                    .await?
            ),
            shuttle_service::Environment::Local => {
                if let Some(local_uri) = self.config.local_uri {
                    DynamoDBOutput::Local(local_uri)
                } else {
                    DynamoDBOutput::Info(
                        factory
                            .get_dynamodb_connection()
                            .await?
                    )
                }
            }
        };

        Ok(info)
    }

    async fn build(build_data: &Self::Output) -> Result<T, shuttle_service::Error> {
        todo!()
        // let connection_string = match build_data {
        //     DbOutput::Local(local_uri) => local_uri.clone(),
        //     DbOutput::Info(info) => info.connection_string_private(),
        // };

        // let pool = $options_path::new()
        //     .min_connections(1)
        //     .max_connections(5)
        //     .connect(&connection_string)
        //     .await
        //     .map_err(CustomError::new)?;

        // Ok(pool)
    }
}

// impl $struct_ident {
//     /// Use a custom connection string for local runs
//     pub fn local_uri(mut self, local_uri: &str) -> Self {
//         self.config.local_uri = Some(local_uri.to_string());

//         self
//     }
// }