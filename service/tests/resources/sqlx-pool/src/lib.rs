use shuttle_service::error::CustomError;
use shuttle_service::Service;
use sqlx::PgPool;

struct PoolService {
    pool: PgPool,
}

#[shuttle_service::main]
async fn init(#[shared::Postgres] pool: PgPool) -> Result<PoolService, shuttle_service::Error> {
    Ok(PoolService { pool })
}

impl PoolService {
    async fn start(&self) -> Result<(), shuttle_service::error::CustomError> {
        let (rec,): (String,) = sqlx::query_as("SELECT 'Hello world'")
            .fetch_one(&self.pool)
            .await
            .map_err(CustomError::new)?;

        assert_eq!(rec, "Hello world");

        Ok(())
    }
}

#[shuttle_service::async_trait]
impl Service for PoolService {
    async fn bind(
        mut self: Box<Self>,
        _: std::net::SocketAddr,
    ) -> Result<(), shuttle_service::error::Error> {
        self.start().await?;

        Ok(())
    }
}
