use async_trait::async_trait;
use serde::{Serialize, Deserialize};
use shuttle_service::{
    database, error::CustomError, DbInput, DbOutput, Error, Factory, ResourceBuilder, Type,
};
use aws_sdk_s3::{Client};

#[derive(Serialize, Deserialize, Clone)]
struct Bucket {
    name: String,
}

pub struct S3 {}

/// Get a `aws_sdk_s3::Client` from any factory
#[async_trait]
impl ResourceBuilder<Bucket> for S3 {
    const TYPE: Type = Type::S3Bucket;

    type Config = ();

    type Output = Bucket;

    fn new() -> Self {
        Self {}
    }

    fn config(&self) -> &Self::Config {
        &()
    }

    async fn output(self, factory: &mut dyn Factory) -> Result<Self::Output, Error> {
        let info = match factory.get_environment() {
            shuttle_service::Environment::Production => Bucket {
                name: factory.get_s3_bucket()
                    .await
                    .map_err(CustomError::new)?,
            },
            shuttle_service::Environment::Local => {
                unimplemented!();
            }
        };
        Ok(info)
    }

    async fn build(build_data: &Self::Output) -> Result<Bucket, Error> {
        Ok(build_data.clone())
    }
}