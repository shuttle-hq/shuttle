use async_trait::async_trait;
use libsql::{Builder, Database};
use serde::{Deserialize, Serialize};
use shuttle_service::{
    error::{CustomError, Error as ShuttleError},
    Environment, IntoResource, ResourceFactory, ResourceInputBuilder,
};
use url::Url;

#[derive(Serialize, Default)]
pub struct Turso {
    addr: String,
    token: String,
    local_addr: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct TursoOutput {
    conn_url: Url,
    token: Option<String>,
    remote: bool,
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
    LocateLocalDB(std::io::Error),
}

impl From<Error> for shuttle_service::Error {
    fn from(error: Error) -> Self {
        let msg = match error {
            Error::UrlParseError(err) => format!("Failed to parse Turso Url: {}", err),
            Error::LocateLocalDB(err) => format!("Failed to get path to local db file: {}", err),
        };

        ShuttleError::Custom(CustomError::msg(msg))
    }
}

impl Turso {
    async fn output_from_addr(
        &self,
        addr: &str,
        remote: bool,
    ) -> Result<TursoOutput, shuttle_service::Error> {
        Ok(TursoOutput {
            conn_url: Url::parse(addr).map_err(Error::UrlParseError)?,
            token: if self.token.is_empty() {
                None
            } else {
                Some(self.token.clone())
            },
            remote,
        })
    }
}

#[async_trait]
impl ResourceInputBuilder for Turso {
    type Input = TursoOutput;
    type Output = TursoOutput;

    async fn build(self, factory: &ResourceFactory) -> Result<Self::Input, ShuttleError> {
        let md = factory.get_metadata();
        match md.env {
            Environment::Deployment => {
                if self.addr.is_empty() {
                    Err(ShuttleError::Custom(CustomError::msg("missing addr")))
                } else {
                    if !self.addr.starts_with("libsql://") && !self.addr.starts_with("https://") {
                        return Err(ShuttleError::Custom(CustomError::msg(
                            "addr must start with either libsql:// or https://",
                        )));
                    }
                    self.output_from_addr(&self.addr, true).await
                }
            }
            Environment::Local => {
                match self.local_addr {
                    Some(ref local_addr) => self.output_from_addr(local_addr, true).await,
                    None => {
                        // Default to a local db of the name of the service.
                        let db_file = std::env::current_dir() // Should be root of the project's workspace
                            .and_then(dunce::canonicalize)
                            .map(|cd| {
                                let mut p = cd.join(md.project_name);
                                p.set_extension("db");
                                p
                            })
                            .map_err(Error::LocateLocalDB)?;
                        let conn_url = format!("file:{}", db_file.display());
                        Ok(TursoOutput {
                            conn_url: Url::parse(&conn_url).map_err(Error::UrlParseError)?,
                            // Nullify the token since we're using a file as database.
                            token: None,
                            remote: false,
                        })
                    }
                }
            }
        }
    }
}

#[async_trait]
impl IntoResource<Database> for TursoOutput {
    async fn into_resource(self) -> Result<Database, shuttle_service::Error> {
        let database = if self.remote {
            Builder::new_remote(
                self.conn_url.to_string(),
                self.token
                    .clone()
                    .ok_or(ShuttleError::Custom(CustomError::msg(
                        "missing token for remote database",
                    )))?,
            )
            .build()
            .await
        } else {
            Builder::new_local(self.conn_url.to_string()).build().await
        };

        database.map_err(|err| ShuttleError::Custom(err.into()))
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[tokio::test]
    async fn local_database_user_supplied() {
        let factory =
            ResourceFactory::new(Default::default(), Default::default(), Default::default());

        let mut turso = Turso::default();
        let local_addr = "libsql://test-addr.turso.io";
        turso = turso.local_addr(local_addr);

        let output = turso.build(&factory).await.unwrap();
        assert_eq!(
            output,
            TursoOutput {
                conn_url: Url::parse(local_addr).unwrap(),
                token: None,
                remote: true,
            }
        )
    }

    #[tokio::test]
    #[should_panic(expected = "missing addr")]
    async fn remote_database_empty_addr() {
        let factory = ResourceFactory::new(
            Default::default(),
            Default::default(),
            Environment::Deployment,
        );

        let turso = Turso::default();
        turso.build(&factory).await.unwrap();
    }

    #[tokio::test]
    async fn remote_database() {
        let factory = ResourceFactory::new(
            Default::default(),
            Default::default(),
            Environment::Deployment,
        );

        let mut turso = Turso::default();
        let addr = "libsql://my-turso-addr.turso.io".to_string();
        turso.addr.clone_from(&addr);
        turso.token = "token".to_string();
        let output = turso.build(&factory).await.unwrap();

        assert_eq!(
            output,
            TursoOutput {
                conn_url: Url::parse(&addr).unwrap(),
                token: Some("token".to_string()),
                remote: true,
            }
        )
    }
}
