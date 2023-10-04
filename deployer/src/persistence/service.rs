use shuttle_common::models::service;
use sqlx::{sqlite::SqliteRow, FromRow, Row};
use ulid::Ulid;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Service {
    pub id: Ulid,
    pub name: String,
}

impl From<Service> for service::Response {
    fn from(service: Service) -> Self {
        Self {
            id: service.id.to_string(),
            name: service.name,
        }
    }
}

impl FromRow<'_, SqliteRow> for Service {
    fn from_row(row: &SqliteRow) -> Result<Self, sqlx::Error> {
        Ok(Self {
            id: Ulid::from_string(row.try_get("id")?).expect("to have a valid ulid string"),
            name: row.try_get("name")?,
        })
    }
}
