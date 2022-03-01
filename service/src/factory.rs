pub trait Factory {
    fn get_sql_connection_pool<D: sqlx::Database>(
        &self,
        name: &str,
    ) -> Result<sqlx::Pool<D>, crate::Error>;
}
