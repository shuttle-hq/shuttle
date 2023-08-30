// This clippy is disabled as per this prost comment
// https://github.com/tokio-rs/prost/issues/661#issuecomment-1156606409
#![allow(clippy::derive_partial_eq_without_eq)]

pub mod provisioner {
    use std::fmt::Display;

    use shuttle_common::{
        database::{self, AwsRdsEngine, SharedEngine},
        DatabaseReadyInfo,
    };

    include!("generated/provisioner.rs");

    impl From<DatabaseResponse> for DatabaseReadyInfo {
        fn from(response: DatabaseResponse) -> Self {
            DatabaseReadyInfo::new(
                response.engine,
                response.username,
                response.password,
                response.database_name,
                response.port,
                response.address_private,
                response.address_public,
            )
        }
    }

    impl From<database::Type> for database_request::DbType {
        fn from(db_type: database::Type) -> Self {
            match db_type {
                database::Type::Shared(engine) => {
                    let engine = match engine {
                        SharedEngine::Postgres => shared::Engine::Postgres(String::new()),
                        SharedEngine::MongoDb => shared::Engine::Mongodb(String::new()),
                    };
                    database_request::DbType::Shared(Shared {
                        engine: Some(engine),
                    })
                }
                database::Type::AwsRds(engine) => {
                    let config = RdsConfig {};
                    let engine = match engine {
                        AwsRdsEngine::Postgres => aws_rds::Engine::Postgres(config),
                        AwsRdsEngine::MariaDB => aws_rds::Engine::Mariadb(config),
                        AwsRdsEngine::MySql => aws_rds::Engine::Mysql(config),
                    };
                    database_request::DbType::AwsRds(AwsRds {
                        engine: Some(engine),
                    })
                }
            }
        }
    }

    impl From<database_request::DbType> for Option<database::Type> {
        fn from(db_type: database_request::DbType) -> Self {
            match db_type {
                database_request::DbType::Shared(Shared {
                    engine: Some(engine),
                }) => match engine {
                    shared::Engine::Postgres(_) => {
                        Some(database::Type::Shared(SharedEngine::Postgres))
                    }
                    shared::Engine::Mongodb(_) => {
                        Some(database::Type::Shared(SharedEngine::MongoDb))
                    }
                },
                database_request::DbType::AwsRds(AwsRds {
                    engine: Some(engine),
                }) => match engine {
                    aws_rds::Engine::Postgres(_) => {
                        Some(database::Type::AwsRds(AwsRdsEngine::Postgres))
                    }
                    aws_rds::Engine::Mysql(_) => Some(database::Type::AwsRds(AwsRdsEngine::MySql)),
                    aws_rds::Engine::Mariadb(_) => {
                        Some(database::Type::AwsRds(AwsRdsEngine::MariaDB))
                    }
                },
                database_request::DbType::Shared(Shared { engine: None })
                | database_request::DbType::AwsRds(AwsRds { engine: None }) => None,
            }
        }
    }

    impl Display for aws_rds::Engine {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Self::Mariadb(_) => write!(f, "mariadb"),
                Self::Mysql(_) => write!(f, "mysql"),
                Self::Postgres(_) => write!(f, "postgres"),
            }
        }
    }
}

pub mod runtime {
    use std::{path::PathBuf, process::Stdio, time::Duration};

    use anyhow::Context;
    use shuttle_common::claims::{
        ClaimLayer, ClaimService, InjectPropagation, InjectPropagationLayer,
    };
    use tokio::process;
    use tonic::transport::{Channel, Endpoint};
    use tower::ServiceBuilder;
    use tracing::{info, trace};

    pub enum StorageManagerType {
        Artifacts(PathBuf),
        WorkingDir(PathBuf),
    }

    include!("generated/runtime.rs");

    pub async fn start(
        wasm: bool,
        storage_manager_type: StorageManagerType,
        provisioner_address: &str,
        logger_uri: &str,
        auth_uri: Option<&String>,
        port: u16,
        get_runtime_executable: impl FnOnce() -> PathBuf,
    ) -> anyhow::Result<(
        process::Child,
        runtime_client::RuntimeClient<ClaimService<InjectPropagation<Channel>>>,
    )> {
        let (storage_manager_type, storage_manager_path) = match storage_manager_type {
            StorageManagerType::Artifacts(path) => ("artifacts", path),
            StorageManagerType::WorkingDir(path) => ("working-dir", path),
        };

        let port = &port.to_string();
        let storage_manager_path = &storage_manager_path.display().to_string();
        let runtime_executable_path = get_runtime_executable();

        let args = if wasm {
            vec!["--port", port]
        } else {
            let mut args = vec![
                "--port",
                port,
                "--provisioner-address",
                provisioner_address,
                "--logger-uri",
                logger_uri,
                "--storage-manager-type",
                storage_manager_type,
                "--storage-manager-path",
                storage_manager_path,
            ];

            if let Some(auth_uri) = auth_uri {
                args.append(&mut vec!["--auth-uri", auth_uri]);
            }

            args
        };

        trace!(
            "Spawning runtime process {:?} {:?}",
            runtime_executable_path,
            args
        );
        let runtime = process::Command::new(runtime_executable_path)
            .args(&args)
            .stdout(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .context("spawning runtime process")?;

        info!("connecting runtime client");
        let conn = Endpoint::new(format!("http://127.0.0.1:{port}"))
            .context("creating runtime client endpoint")?
            .connect_timeout(Duration::from_secs(5));

        // Wait for the spawned process to open the endpoint port.
        // Connecting instantly does not give it enough time.
        let channel = tokio::time::timeout(Duration::from_millis(7000), async move {
            let mut ms = 5;
            loop {
                if let Ok(channel) = conn.connect().await {
                    break channel;
                }
                trace!("waiting for runtime endpoint to open");
                // exponential backoff
                tokio::time::sleep(Duration::from_millis(ms)).await;
                ms *= 2;
            }
        })
        .await
        .context("runtime client endpoint did not open in time")?;

        let channel = ServiceBuilder::new()
            .layer(ClaimLayer)
            .layer(InjectPropagationLayer)
            .service(channel);
        let runtime_client = runtime_client::RuntimeClient::new(channel);

        Ok((runtime, runtime_client))
    }
}

pub mod resource_recorder {
    use std::str::FromStr;

    include!("generated/resource_recorder.rs");

    impl From<record_request::Resource> for shuttle_common::resource::Response {
        fn from(resource: record_request::Resource) -> Self {
            shuttle_common::resource::Response {
                r#type: shuttle_common::resource::Type::from_str(resource.r#type.as_str())
                    .expect("to have a valid resource string"),
                config: serde_json::from_slice(&resource.config)
                    .expect("to have JSON valid config"),
                data: serde_json::from_slice(&resource.data).expect("to have JSON valid data"),
            }
        }
    }

    impl From<Resource> for shuttle_common::resource::Response {
        fn from(resource: Resource) -> Self {
            shuttle_common::resource::Response {
                r#type: shuttle_common::resource::Type::from_str(resource.r#type.as_str())
                    .expect("to have a valid resource string"),
                config: serde_json::from_slice(&resource.config)
                    .expect("to have JSON valid config"),
                data: serde_json::from_slice(&resource.data).expect("to have JSON valid data"),
            }
        }
    }
}

pub mod logger {
    use std::time::Duration;

    use prost::bytes::Bytes;
    use tokio::{select, sync::mpsc, time::interval};
    use tonic::{
        async_trait,
        codegen::{Body, StdError},
        Request,
    };
    use tracing::error;

    use self::logger_client::LoggerClient;

    include!("generated/logger.rs");

    /// Adapter to some client which expects to receive a vector of items
    #[async_trait]
    pub trait VecReceiver: Send {
        type Item;

        async fn receive(&mut self, items: Vec<Self::Item>);
    }

    #[async_trait]
    impl<T> VecReceiver for LoggerClient<T>
    where
        T: tonic::client::GrpcService<tonic::body::BoxBody> + Send + Sync + Clone,
        T::Error: Into<StdError>,
        T::ResponseBody: Body<Data = Bytes> + Send + 'static,
        T::Future: Send,
        <T::ResponseBody as Body>::Error: Into<StdError> + Send,
    {
        type Item = LogItem;

        async fn receive(&mut self, items: Vec<Self::Item>) {
            match self
                .store_logs(Request::new(StoreLogsRequest { logs: items }))
                .await
            {
                Ok(_) => {}
                Err(error) => error!(
                    error = &error as &dyn std::error::Error,
                    "failed to send batch logs to logger"
                ),
            }
        }
    }

    /// Wrapper to batch together items before forwarding them to some vector receiver
    #[derive(Clone)]
    pub struct Batcher<I: VecReceiver> {
        tx: mpsc::UnboundedSender<I::Item>,
    }

    impl<I: VecReceiver + 'static> Batcher<I>
    where
        I::Item: Send,
    {
        /// Create a new batcher around inner with the given batch capacity.
        /// Items will be send when the batch has reached capacity or at the set interval. Whichever comes first.
        pub fn new(inner: I, capacity: usize, interval: Duration) -> Self {
            let (tx, rx) = mpsc::unbounded_channel();

            tokio::spawn(Self::batch(inner, rx, capacity, interval));

            Self { tx }
        }

        /// Create a batcher around inner. It will send a batch of items to inner if a capacity of 2048 is reached
        /// or if an interval of 5 seconds are reached.
        ///
        /// These are the same defaults used by the otel batcher
        pub fn wrap(inner: I) -> Self {
            Self::new(inner, 2048, Duration::from_secs(5))
        }

        /// Send a single item into this batcher
        pub fn send(&self, item: I::Item) {
            match self.tx.send(item) {
                Ok(_) => {}
                Err(_) => unreachable!("the receiver will never drop"),
            }
        }

        /// Background task to forward the items ones the batch capacity has been reached
        async fn batch(
            mut inner: I,
            mut rx: mpsc::UnboundedReceiver<I::Item>,
            capacity: usize,
            interval_duration: Duration,
        ) {
            let mut interval = interval(interval_duration);

            // Without this, the default behaviour will burst any missed tickers until they are caught up.
            // This will cause a flood which we want to avoid.
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

            // Get past the first tick
            interval.tick().await;

            let mut cache = Vec::with_capacity(capacity);

            loop {
                select! {
                    item = rx.recv() => {
                        if let Some(item) = item {
                            cache.push(item);

                            if cache.len() == capacity {
                                let old_cache = cache;
                                cache = Vec::with_capacity(capacity);

                                inner.receive(old_cache).await;
                            }
                        } else {
                            // Sender dropped
                            return;
                        }
                    },
                    _ = interval.tick() => {
                        if !cache.is_empty() {
                            let old_cache = cache;
                            cache = Vec::with_capacity(capacity);

                            inner.receive(old_cache).await;
                        }
                    }
                }
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use std::{
            sync::{Arc, Mutex},
            time::Duration,
        };

        use tokio::time::sleep;
        use tonic::async_trait;

        use super::{Batcher, VecReceiver};

        #[derive(Default, Clone)]
        struct MockGroupReceiver(Arc<Mutex<Option<Vec<u32>>>>);

        #[async_trait]
        impl VecReceiver for MockGroupReceiver {
            type Item = u32;

            async fn receive(&mut self, items: Vec<Self::Item>) {
                *self.0.lock().unwrap() = Some(items);
            }
        }

        #[tokio::test(flavor = "multi_thread")]
        async fn capacity_reached() {
            let mock = MockGroupReceiver::default();
            let batcher = Batcher::new(mock.clone(), 2, Duration::from_secs(120));

            batcher.send(1);
            sleep(Duration::from_millis(50)).await;
            assert_eq!(*mock.0.lock().unwrap(), None);

            batcher.send(2);
            sleep(Duration::from_millis(50)).await;
            assert_eq!(*mock.0.lock().unwrap(), Some(vec![1, 2]));

            batcher.send(3);
            sleep(Duration::from_millis(50)).await;
            assert_eq!(*mock.0.lock().unwrap(), Some(vec![1, 2]));

            batcher.send(4);
            sleep(Duration::from_millis(50)).await;
            assert_eq!(*mock.0.lock().unwrap(), Some(vec![3, 4]));
        }

        #[tokio::test(flavor = "multi_thread")]
        async fn interval_reached() {
            let mock = MockGroupReceiver::default();
            let batcher = Batcher::new(mock.clone(), 2, Duration::from_millis(300));

            sleep(Duration::from_millis(500)).await;
            assert_eq!(
                *mock.0.lock().unwrap(),
                None,
                "we should never send something when the cache is empty"
            );

            batcher.send(1);
            sleep(Duration::from_millis(50)).await;
            assert_eq!(*mock.0.lock().unwrap(), None);

            sleep(Duration::from_millis(500)).await;
            assert_eq!(*mock.0.lock().unwrap(), Some(vec![1]));
        }
    }
}
