use shuttle_service::error::CustomError;
use shuttle_service::{Factory, IntoService, Service};
use sqlx::PgPool;
use tokio::runtime::Runtime;

#[macro_use]
extern crate shuttle_service;

struct Args;

struct PoolService {
    runtime: Runtime,
    pool: Option<PgPool>,
}

fn init() -> Args {
    Args
}

impl IntoService for Args {
    type Service = PoolService;

    fn into_service(self) -> Self::Service {
        PoolService {
            pool: None,
            runtime: Runtime::new().unwrap(),
        }
    }
}

impl PoolService {
    async fn start(&self) -> Result<(), shuttle_service::error::Error> {
        if let Some(pool) = &self.pool {
            let (rec,): (String,) = sqlx::query_as("SELECT 'Hello world'")
                .fetch_one(pool)
                .await
                .map_err(CustomError::new)?;

            assert_eq!(rec, "Hello world");
        } else {
            panic!("we should have an active pool");
        }

        Ok(())
    }
}

impl Service for PoolService {
    fn bind(&mut self, _: std::net::SocketAddr) -> Result<(), shuttle_service::error::Error> {
        self.runtime.block_on(self.start())?;

        Ok(())
    }

    fn build(
        &mut self,
        factory: &mut dyn shuttle_service::Factory,
    ) -> Result<(), shuttle_service::Error> {
        let pool = self
            .runtime
            .block_on(get_postgres_connection_pool(factory))?;

        self.pool = Some(pool);

        Ok(())
    }
}

async fn get_postgres_connection_pool(
    factory: &mut dyn Factory,
) -> Result<PgPool, shuttle_service::error::Error> {
    let connection_string = factory.get_sql_connection_string().await?;
    let pool = sqlx::postgres::PgPoolOptions::new()
        .min_connections(1)
        .max_connections(5)
        .connect(&connection_string)
        .await
        .map_err(CustomError::new)?;

    Ok(pool)
}

declare_service!(Args, init);
