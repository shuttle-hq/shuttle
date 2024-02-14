use async_trait::async_trait;
use opendal::{Operator, Scheme};
use serde::{Deserialize, Serialize};
use shuttle_service::{
    error::{CustomError, Error as ShuttleError},
    resource::Type,
    Factory, IntoResource, ResourceBuilder, Secret,
};
use std::collections::{BTreeMap, HashMap};
use std::str::FromStr;

#[derive(Serialize)]
pub struct Opendal {
    scheme: String,
}

impl Default for Opendal {
    fn default() -> Self {
        Self {
            scheme: "memory".to_string(),
        }
    }
}

impl Opendal {
    pub fn scheme(mut self, scheme: &str) -> Self {
        self.scheme = scheme.to_string();
        self
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct OpendalOutput {
    scheme: String,
    cfg: BTreeMap<String, Secret<String>>,
}

pub struct Error(opendal::Error);

impl From<Error> for shuttle_service::Error {
    fn from(error: Error) -> Self {
        let msg = format!("Failed to build opendal resource: {:?}", error.0);
        ShuttleError::Custom(CustomError::msg(msg))
    }
}

#[async_trait]
impl ResourceBuilder for Opendal {
    const TYPE: Type = Type::Custom;
    type Config = ();
    type Output = OpendalOutput;

    fn config(&self) -> &Self::Config {
        &()
    }

    async fn output(
        self,
        factory: &mut dyn Factory,
    ) -> Result<Self::Output, shuttle_service::Error> {
        let scheme = self.scheme;

        // Expose secrets from resource factory, MAKE SURE it not leaked into log.
        let cfg = factory.get_secrets().await?.clone();
        Ok(OpendalOutput { scheme, cfg })
    }
}

#[async_trait]
impl IntoResource<Operator> for OpendalOutput {
    async fn into_resource(self) -> Result<Operator, shuttle_service::Error> {
        let (scheme, cfg) = (self.scheme, self.cfg);

        let scheme = Scheme::from_str(&scheme).map_err(Error)?;

        // Expose secrets from resource factory, MAKE SURE it not leaked into log.
        let cfg: HashMap<_, _> = cfg
            .into_iter()
            .map(|(k, v)| (k, v.expose().clone()))
            .collect();

        let op = Operator::via_map(scheme, cfg).map_err(Error)?;

        Ok(op)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use shuttle_service::{Environment, Secret};

    struct MockFactory {
        pub secrets: BTreeMap<String, String>,
    }

    #[async_trait]
    impl Factory for MockFactory {
        async fn get_db_connection(
            &mut self,
            _db_type: shuttle_service::database::Type,
        ) -> Result<shuttle_service::DatabaseInfo, shuttle_service::Error> {
            panic!("no opendal test should try to get a db connection string")
        }

        async fn get_container(
            &mut self,
            _req: shuttle_service::ContainerRequest,
        ) -> Result<shuttle_service::ContainerResponse, shuttle_service::Error> {
            panic!("no opendal test should try to get a container")
        }

        async fn get_secrets(
            &mut self,
        ) -> Result<BTreeMap<String, Secret<String>>, shuttle_service::Error> {
            let secrets = self
                .secrets
                .iter()
                .map(|(k, v)| (k.clone(), Secret::new(v.clone())))
                .collect();
            Ok(secrets)
        }

        fn get_metadata(&self) -> shuttle_service::DeploymentMetadata {
            shuttle_service::DeploymentMetadata {
                env: Environment::Local,
                project_name: "my-opendal-service".to_string(),
                service_name: "my-opendal-service".to_string(),
                storage_path: std::path::PathBuf::new(),
            }
        }
    }

    #[tokio::test]
    async fn opendal_fs() {
        let mut factory = MockFactory {
            secrets: [("root", "/tmp")]
                .into_iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
        };

        let odal = Opendal::default().scheme("fs");
        let output = odal.output(&mut factory).await.unwrap();
        assert_eq!(output.scheme, "fs");

        let op: Operator = output.into_resource().await.unwrap();
        assert_eq!(op.info().scheme(), Scheme::Fs)
    }

    #[tokio::test]
    async fn opendal_s3() {
        let mut factory = MockFactory {
            secrets: [
                ("bucket", "test"),
                ("access_key_id", "ak"),
                ("secret_access_key", "sk"),
                ("region", "us-east-1"),
            ]
            .into_iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
        };

        let odal = Opendal::default().scheme("s3");
        let output = odal.output(&mut factory).await.unwrap();
        assert_eq!(output.scheme, "s3");
        assert_eq!(output.cfg.get("access_key_id").unwrap().expose(), "ak");
        assert_eq!(output.cfg.get("secret_access_key").unwrap().expose(), "sk");

        let op: Operator = output.into_resource().await.unwrap();
        assert_eq!(op.info().scheme(), Scheme::S3)
    }
}
