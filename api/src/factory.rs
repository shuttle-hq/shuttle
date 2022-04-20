use crate::database;
use async_trait::async_trait;
use shuttle_service::Factory;

pub(crate) struct ShuttleFactory<'a> {
    database: &'a mut database::State,
}

impl<'a> ShuttleFactory<'a> {
    pub(crate) fn new(database: &'a mut database::State) -> Self {
        Self { database }
    }
}

#[async_trait]
impl Factory for ShuttleFactory<'_> {
    async fn get_sql_connection_string(&mut self) -> Result<String, shuttle_service::Error> {
        let conn_str = self.database.request().connection_string("localhost");
        debug!("giving a sql connection string: {}", conn_str);
        Ok(conn_str)
    }
}
