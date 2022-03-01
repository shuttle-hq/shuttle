pub trait Factory: Send + Sync {
    fn get_postgres_connection_pool(&self, name: &str) -> Result<sqlx::PgPool, crate::Error>;
}

impl Factory for Box<dyn Factory> {
    fn get_postgres_connection_pool(&self, name: &str) -> Result<sqlx::PgPool, crate::Error> {
        self.as_ref().get_postgres_connection_pool(name)
    }
}
