use std::net::Ipv4Addr;
use std::str::FromStr;

use sqlx::types::Json as SqlxJson;
use sqlx::Row;
use sqlx::{sqlite::SqliteRow, FromRow};
use tracing::error;
use ulid::Ulid;

use super::error::Error;
use crate::project::service::ServiceState;

// User service model from persistence.
#[derive(Clone, Debug, PartialEq)]
pub struct Service {
    pub id: Ulid,
    pub name: String,
    pub state_variant: String,
    pub state: ServiceState,
}

impl Service {
    pub fn target_ip(&self, network_name: &str) -> Result<Ipv4Addr, Error> {
        match self.state.container() {
            Some(inner) => match inner.network_settings {
                Some(network) => match network.networks.as_ref() {
                    Some(net) => {
                        let ip = net
                            .get(network_name)
                            .expect("to be attached to the network")
                            .ip_address
                            .as_ref()
                            .expect("to have an IP address");
                        Ipv4Addr::from_str(ip.as_str())
                            .map_err(|err| Error::FieldNotFound(err.to_string()))
                    }
                    None => {
                        error!("ip address not found on the network setting of the service {} container", self.id);
                        Err(Error::FieldNotFound(format!("service {} address", self.id)))
                    }
                },
                None => {
                    error!(
                        "missing network settings on the service {} container",
                        self.id
                    );
                    Err(Error::FieldNotFound(format!("service {} address", self.id)))
                }
            },
            None => {
                error!(
                    "missing container inspect information for service {}",
                    self.id
                );
                Err(Error::FieldNotFound(format!("service {} address", self.id)))
            }
        }
    }
}

impl FromRow<'_, SqliteRow> for Service {
    fn from_row(row: &SqliteRow) -> Result<Self, sqlx::Error> {
        Ok(Self {
            id: Ulid::from_string(row.try_get("id")?)
                .map_err(|e| sqlx::Error::Decode(Box::new(e)))?,
            name: row.try_get("name")?,
            state_variant: row.try_get("state_variant")?,
            state: row.try_get::<SqlxJson<ServiceState>, _>("state")?.0,
        })
    }
}
