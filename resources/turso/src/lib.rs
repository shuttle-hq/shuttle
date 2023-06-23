use async_trait::async_trait;
use libsql_client::{client::Client, new_client_from_config, Config};
use serde::{Deserialize, Serialize};
use shuttle_service::{
    error::{CustomError, Error as ShuttleError},
    Factory, ResourceBuilder, Type,
};
use url::Url;

#[derive(Serialize, Deserialize, Default)]
pub struct Turso {
    addr: String,
    token: String,
    local_addr: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct TursoOutput {
    conn_url: Url,
    token: Option<String>,
}

impl Turso {
    pub fn addr(mut self, addr: &str) -> Self {
        self.addr = addr.to_string();
        self
    }

    pub fn token(mut self, token: &str) -> Self {
        self.token = token.to_string();
        self
    }

    pub fn local_addr(mut self, local_addr: &str) -> Self {
        self.local_addr = Some(local_addr.to_string());
        self
    }
}

pub enum Error {
    UrlParseError(url::ParseError),
}

impl From<Error> for shuttle_service::Error {
    fn from(error: Error) -> Self {
        let msg = match error {
            Error::UrlParseError(err) => format!("Cannot parse Turso Url: {}", err),
        };

        ShuttleError::Custom(CustomError::msg(msg))
    }
}

impl Turso {
    async fn output_from_addr(
        &self,
        addr: &str,
    ) -> Result<<Turso as ResourceBuilder<Client>>::Output, shuttle_service::Error> {
        Ok(TursoOutput {
            conn_url: Url::parse(addr).map_err(Error::UrlParseError)?,
            token: if self.token.is_empty() {
                None
            } else {
                Some(self.token.clone())
            },
        })
    }
}

#[async_trait]
impl ResourceBuilder<Client> for Turso {
    const TYPE: Type = Type::Turso;

    type Config = Self;
    type Output = TursoOutput;

    fn new() -> Self {
        Self::default()
    }

    fn config(&self) -> &Self::Config {
        self
    }

    async fn output(
        self,
        factory: &mut dyn Factory,
    ) -> Result<Self::Output, shuttle_service::Error> {
        match factory.get_environment() {
            shuttle_service::Environment::Production => {
                if self.addr.is_empty() {
                    Err(ShuttleError::Custom(CustomError::msg("missing addr")))
                } else {
                    let addr = if self.addr.starts_with("libsql://") {
                        self.addr.to_string()
                    } else {
                        format!("libsql://{}", self.addr)
                    };
                    self.output_from_addr(&addr).await
                }
            }
            shuttle_service::Environment::Local => {
                // Default to a local db of the name of the service.
                let default_db_path = factory
                    .get_build_path()?
                    .join(format!("{}.db", factory.get_service_name()));

                match self.local_addr {
                    Some(ref local_addr) => self.output_from_addr(local_addr).await,
                    None => {
                        let conn_url = format!(
                            "file://{}",
                            default_db_path
                                .to_str()
                                .expect("local db should be a valid unicode string")
                        );
                        Ok(TursoOutput {
                            conn_url: Url::parse(&conn_url).map_err(Error::UrlParseError)?,
                            // Nullify the token since we're using a file as database.
                            token: None,
                        })
                    }
                }
            }
        }
    }

    async fn build(config: &Self::Output) -> Result<Client, shuttle_service::Error> {
        let client = new_client_from_config(Config {
            url: config.conn_url.clone(),
            auth_token: config.token.clone(),
        })
        .await?;
        Ok(client)
    }
}

#[cfg(test)]
mod test {

    use std::path::PathBuf;
    use std::{fs, str::FromStr};

    use async_trait::async_trait;
    use shuttle_service::{DatabaseReadyInfo, Environment, Factory, ResourceBuilder, ServiceName};
    use tempfile::{Builder, TempDir};
    use url::Url;

    use crate::{Turso, TursoOutput};

    struct MockFactory {
        temp_dir: TempDir,
        pub service_name: String,
        pub environment: Environment,
    }

    impl MockFactory {
        fn new() -> Self {
            Self {
                temp_dir: Builder::new().prefix("shuttle-turso").tempdir().unwrap(),
                service_name: "shuttle-turso".to_string(),
                environment: Environment::Local,
            }
        }

        fn build_path(&self) -> PathBuf {
            self.get_path("build")
        }

        fn get_path(&self, folder: &str) -> PathBuf {
            let path = self.temp_dir.path().join(folder);

            if !path.exists() {
                fs::create_dir(&path).unwrap();
            }

            path
        }
    }

    #[async_trait]
    impl Factory for MockFactory {
        async fn get_db_connection(
            &mut self,
            _db_type: shuttle_service::database::Type,
        ) -> Result<DatabaseReadyInfo, shuttle_service::Error> {
            panic!("no turso test should try to get a db connection string")
        }

        async fn get_secrets(
            &mut self,
        ) -> Result<std::collections::BTreeMap<String, String>, shuttle_service::Error> {
            panic!("no turso test should try to get secrets")
        }

        fn get_service_name(&self) -> shuttle_service::ServiceName {
            ServiceName::from_str(&self.service_name).unwrap()
        }

        fn get_environment(&self) -> shuttle_service::Environment {
            self.environment
        }

        fn get_build_path(&self) -> Result<std::path::PathBuf, shuttle_service::Error> {
            Ok(self.build_path())
        }

        fn get_storage_path(&self) -> Result<std::path::PathBuf, shuttle_service::Error> {
            panic!("no turso test should try to get the storage path")
        }
    }

    #[tokio::test]
    async fn local_database_default() {
        let mut factory = MockFactory::new();

        let turso = Turso::new();
        let output = turso.output(&mut factory).await.unwrap();
        assert_eq!(
            output,
            TursoOutput {
                conn_url: Url::parse(&format!(
                    "file:///{}/shuttle-turso.db",
                    factory.get_build_path().unwrap().to_str().unwrap()
                ))
                .unwrap(),
                token: None
            }
        )
    }

    #[tokio::test]
    async fn local_database_user_supplied() {
        let mut factory = MockFactory::new();

        let mut turso = Turso::new();
        let local_addr = "libsql://test-addr.turso.io";
        turso = turso.local_addr(local_addr);

        let output = turso.output(&mut factory).await.unwrap();
        assert_eq!(
            output,
            TursoOutput {
                conn_url: Url::parse(local_addr).unwrap(),
                token: None
            }
        )
    }

    #[tokio::test]
    #[should_panic(expected = "missing addr")]
    async fn remote_database_empty_addr() {
        let mut factory = MockFactory::new();
        factory.environment = Environment::Production;

        let turso = Turso::new();
        turso.output(&mut factory).await.unwrap();
    }

    #[tokio::test]
    async fn remote_database() {
        let mut factory = MockFactory::new();
        factory.environment = Environment::Production;

        let mut turso = Turso::new();
        let addr = "my-turso-addr.turso.io".to_string();
        turso.addr = addr.clone();
        turso.token = "token".to_string();
        let output = turso.output(&mut factory).await.unwrap();

        assert_eq!(
            output,
            TursoOutput {
                conn_url: Url::parse(&format!("libsql://{}", addr)).unwrap(),
                token: Some("token".to_string())
            }
        )
    }
}
