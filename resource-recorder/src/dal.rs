use crate::r#type::Type;
use async_trait::async_trait;
use uuid::Uuid;

#[async_trait]
pub trait Dal {
    async fn add_resources(&self, resources: Vec<Resource>);
}

pub struct Sqlite;

#[async_trait]
impl Dal for Sqlite {
    async fn add_resources(&self, resources: Vec<Resource>) {
        todo!();
    }
}

#[derive(sqlx::FromRow, Debug, Eq, PartialEq)]
pub struct Resource {
    pub id: Uuid,
    pub service_id: Uuid,
    pub r#type: Type,
    pub data: serde_json::Value,
    pub config: serde_json::Value,
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use uuid::Uuid;

    use crate::{
        dal::{Dal, Resource},
        r#type::Type,
    };

    use super::Sqlite;

    #[tokio::test]
    async fn add_resource() {
        let dal = Sqlite;
        let service_id = Uuid::new_v4();

        let database = Resource {
            id: Uuid::new_v4(),
            service_id,
            r#type: Type::Database(crate::r#type::database::Type::Shared(
                crate::r#type::database::SharedType::Postgres,
            )),
            config: json!({"private": false}),
            data: json!({"username": "test"}),
        };
        let secrets = Resource {
            id: Uuid::new_v4(),
            service_id,
            r#type: Type::Secrets,
            config: json!({}),
            data: json!({"password": "p@ssw0rd"}),
        };
        let static_folder = Resource {
            id: Uuid::new_v4(),
            service_id,
            r#type: Type::StaticFolder,
            config: json!({"path": "static"}),
            data: json!({"path": "/tmp/static"}),
        };

        dal.add_resources(vec![database, secrets, static_folder])
            .await;
    }
}
