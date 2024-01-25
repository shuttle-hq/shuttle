pub mod database;

use shuttle_common::{claims::Claim, resource::Type};
use shuttle_proto::resource_recorder::{
    record_request, ResourceResponse, ResourcesResponse, ResultResponse,
};
use sqlx::{sqlite::SqliteRow, FromRow, Row};
use ulid::Ulid;

pub use self::database::Type as DatabaseType;

#[async_trait::async_trait]
pub trait ResourceManager: Clone + Send + Sync + 'static {
    type Err: std::error::Error;

    async fn insert_resources(
        &mut self,
        resources: Vec<record_request::Resource>,
        service_id: &ulid::Ulid,
        claim: Claim,
    ) -> Result<ResultResponse, Self::Err>;
    async fn get_resources(
        &mut self,
        service_id: &ulid::Ulid,
        claim: Claim,
    ) -> Result<ResourcesResponse, Self::Err>;
    async fn get_resource(
        &mut self,
        service_id: &ulid::Ulid,
        r#type: Type,
        claim: Claim,
    ) -> Result<ResourceResponse, Self::Err>;
    async fn delete_resource(
        &mut self,
        project_name: String,
        service_id: &ulid::Ulid,
        r#type: Type,
        claim: Claim,
    ) -> Result<ResultResponse, Self::Err>;
}

#[derive(Debug, Eq, PartialEq)]
pub struct Resource {
    pub service_id: Ulid,
    pub r#type: Type,
    pub data: serde_json::Value,
    pub config: serde_json::Value,
}

impl FromRow<'_, SqliteRow> for Resource {
    fn from_row(row: &SqliteRow) -> Result<Self, sqlx::Error> {
        Ok(Self {
            service_id: Ulid::from_string(row.try_get("service_id")?)
                .expect("to have a valid ulid string"),
            r#type: row.try_get("type")?,
            data: row.try_get("data")?,
            config: row.try_get("config")?,
        })
    }
}

impl From<Resource> for shuttle_common::resource::Response {
    fn from(resource: Resource) -> Self {
        shuttle_common::resource::Response {
            r#type: resource.r#type,
            config: resource.config,
            data: resource.data,
        }
    }
}
