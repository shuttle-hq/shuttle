pub(crate) enum DatabaseState {
    Uninitialised,
    Initialised(())
}


impl DatabaseState {
    pub(crate) fn get_client(&self) -> Result<sqlx::PgPool, unveil_service::Error> {

        unimplemented!()
    }
}