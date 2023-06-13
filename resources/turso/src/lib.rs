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

#[derive(Serialize, Deserialize)]
pub struct TursoOutput {
    conn_url: Url,
    token: Option<String>,
}

impl Turso {
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

#[async_trait]
impl ResourceBuilder<Client> for Turso {
    const TYPE: Type = Type::Turso;

    type Config = Self;
    type Output = TursoOutput;

    fn new() -> Self {
        Self::default()
    }

    fn config(&self) -> &Self::Config {
        &self
    }

    async fn output(
        self,
        factory: &mut dyn Factory,
    ) -> Result<Self::Output, shuttle_service::Error> {
        match factory.get_environment() {
            shuttle_service::Environment::Production => {
                if self.addr.is_empty() {
                    // XXX: should we raise an error even not running in a production
                    // environment ?
                    Err(ShuttleError::Custom(CustomError::msg("missing addr")))
                } else {
                    Ok(TursoOutput {
                        // XXX: should we allow for other kind of connection string ? Not having libsql at
                        // the start of the url even though the Turso CLI is printing it might be confusing
                        // instead of just giving the responsability to the user.
                        conn_url: Url::parse(&format!("libsql://{}", self.addr))
                            .map_err(Error::UrlParseError)?,
                        token: Some(self.token),
                    })
                }
            }
            shuttle_service::Environment::Local => {
                // Default to a local db of the name of the service.
                let default_db_path = factory
                    .get_build_path()?
                    .join(format!("{}.db", factory.get_service_name()));

                let conn_url = self.local_addr.unwrap_or(format!(
                    "file://{}",
                    default_db_path
                        .to_str()
                        .expect("local db should be a valid unicode string")
                ));
                Ok(TursoOutput {
                    conn_url: Url::parse(&conn_url).map_err(Error::UrlParseError)?,
                    token: None,
                })
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
