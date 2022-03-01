use unveil_service::Factory;

pub(crate) struct UnveilFactory;

impl Factory for UnveilFactory {
    fn get_postgres_connection_pool(
        &self,
        _name: &str,
    ) -> Result<sqlx::PgPool, unveil_service::Error> {
        todo!()
    }
}
