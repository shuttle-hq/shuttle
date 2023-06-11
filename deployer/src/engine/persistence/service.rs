use shuttle_common::models::service;
use ulid::Ulid;

#[derive(Clone, Debug, Eq, PartialEq, sqlx::FromRow)]
pub struct Service {
    pub id: Ulid,
    pub name: String,
}

impl From<Service> for service::Response {
    fn from(service: Service) -> Self {
        Self {
            id: service.id,
            name: service.name,
        }
    }
}
