use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use shuttle_service::{database, Factory, ResourceBuilder, Type};

// Builder struct
#[derive(Serialize)]
pub struct SQLite {}

// Resource struct
#[derive(Deserialize, Serialize, Clone)]
pub struct SQLiteInstance {}

#[async_trait]
impl ResourceBuilder<SQLiteInstance> for SQLite {
    const TYPE: Type = Type::Database(database::Type::Filesystem);

    type Config = Self;

    type Output = SQLiteInstance;

    fn new() -> Self {
        Self {}
    }

    fn config(&self) -> &Self::Config {
        &self
    }

    async fn output(
        self,
        _factory: &mut dyn Factory,
    ) -> Result<Self::Output, shuttle_service::Error> {
        Ok(SQLiteInstance {})
    }

    async fn build(build_data: &Self::Output) -> Result<SQLiteInstance, shuttle_service::Error> {
        Ok(build_data.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        //
    }
}
