use async_broadcast::{broadcast, Sender};
use async_trait::async_trait;
use sqlx::{postgres::PgConnectOptions, PgPool, QueryBuilder};
use tonic::transport::Uri;

use crate::dal::{Dal, DalError, Log, MIGRATIONS};

#[derive(Clone)]
pub struct Postgres {
    pool: PgPool,
    tx: Sender<Vec<Log>>,
}

impl Postgres {
    /// This function creates all necessary tables and sets up a database connection pool.
    pub async fn new(connection_uri: &Uri) -> Self {
        let pool = PgPool::connect(connection_uri.to_string().as_str())
            .await
            .expect("to be able to connect to the postgres db using the connection url");
        Self::from_pool(pool).await
    }

    pub async fn with_options(options: PgConnectOptions) -> Self {
        let pool = PgPool::connect_with(options)
            .await
            .expect("to be able to connect to the postgres db using the pg connect options");
        Self::from_pool(pool).await
    }

    async fn from_pool(pool: PgPool) -> Self {
        MIGRATIONS
            .run(&pool)
            .await
            .expect("to run migrations successfully");

        let (tx, mut rx) = broadcast(256);
        let pool_spawn = pool.clone();
        tokio::spawn(async move {
            while let Ok(logs) = rx.recv().await {
                let mut builder = QueryBuilder::new("INSERT INTO logs(deployment_id, shuttle_service_name, timestamp, level, fields) ");
                builder.push_values(logs, |mut b, log: Log| {
                    b.push_bind(log.deployment_id)
                        .push_bind(log.shuttle_service_name)
                        .push_bind(log.timestamp)
                        .push_bind(log.level)
                        .push_bind(log.fields);
                });
                let query = builder.build();
                query
                    .execute(&pool_spawn)
                    .await
                    .expect("to be able to execute the query for log insertion");
            }
        });

        Self { pool, tx }
    }

    /// Get the sender to broadcast logs into
    pub fn get_sender(&self) -> Sender<Vec<Log>> {
        self.tx.clone()
    }
}

#[async_trait]
impl Dal for Postgres {
    async fn get_logs(&self, deployment_id: String) -> Result<Vec<Log>, DalError> {
        let result = sqlx::query_as("SELECT * FROM logs WHERE deployment_id = $1")
            .bind(deployment_id)
            .fetch_all(&self.pool)
            .await?;

        Ok(result)
    }
}
