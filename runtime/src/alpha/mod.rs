use std::{
    collections::BTreeMap,
    iter::FromIterator,
    net::{Ipv4Addr, SocketAddr},
    ops::DerefMut,
    str::FromStr,
    sync::{Arc, Mutex},
    time::Duration,
};

use anyhow::Context;
use async_trait::async_trait;
use core::future::Future;
use shuttle_common::{
    backends::{
        auth::{AuthPublicKey, JwtAuthenticationLayer},
        tracing::ExtractPropagationLayer,
    },
    claims::{Claim, ClaimLayer, InjectPropagationLayer},
    resource,
    storage_manager::{ArtifactsStorageManager, StorageManager, WorkingDirStorageManager},
};
use shuttle_proto::{
    provisioner::provisioner_client::ProvisionerClient,
    runtime::{
        self,
        runtime_server::{Runtime, RuntimeServer},
        LoadRequest, LoadResponse, LogItem, StartRequest, StartResponse, StopReason, StopRequest,
        StopResponse, SubscribeLogsRequest, SubscribeStopRequest, SubscribeStopResponse,
    },
};
use shuttle_service::{Environment, Factory, Service, ServiceName};
use tokio::sync::{broadcast, oneshot};
use tokio::sync::{
    broadcast::Sender,
    mpsc::{self, UnboundedReceiver, UnboundedSender},
};
use tokio_stream::wrappers::ReceiverStream;
use tonic::{
    transport::{Endpoint, Server},
    Request, Response, Status,
};
use tower::ServiceBuilder;
use tracing::{error, info, trace, warn};

use crate::{provisioner_factory::ProvisionerFactory, Logger, ResourceTracker};

use self::args::Args;

mod args;

pub async fn start(loader: impl Loader<ProvisionerFactory> + Send + 'static) {
    let args = match Args::parse() {
        Ok(args) => args,
        Err(_) => {
            let help_str = "[HINT]: Run shuttle with `cargo shuttle run`";
            let wrapper_str = "-".repeat(help_str.len());
            return println!("{wrapper_str}\n{help_str}\n{wrapper_str}",);
        }
    };

    let addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), args.port);

    let provisioner_address = args.provisioner_address;
    let mut server_builder = Server::builder()
        .http2_keepalive_interval(Some(Duration::from_secs(60)))
        .layer(JwtAuthenticationLayer::new(AuthPublicKey::new(
            args.auth_uri,
        )))
        .layer(ExtractPropagationLayer);

    // We wrap the StorageManager trait object in an Arc rather than a Box, since we need
    // to clone it in the `ProvisionerFactory::new` call in the alpha runtime `load` method.
    // We might be able to optimize this by implementing clone for a Box<dyn StorageManager>
    // or by using static dispatch instead.
    let (storage_manager, env): (Arc<dyn StorageManager>, Environment) =
        match args.storage_manager_type {
            args::StorageManagerType::Artifacts => (
                Arc::new(ArtifactsStorageManager::new(args.storage_manager_path)),
                Environment::Production,
            ),
            args::StorageManagerType::WorkingDir => (
                Arc::new(WorkingDirStorageManager::new(args.storage_manager_path)),
                Environment::Local,
            ),
        };

    let router = {
        let alpha = Alpha::new(provisioner_address, loader, storage_manager, env);

        let svc = RuntimeServer::new(alpha);
        server_builder.add_service(svc)
    };

    match router.serve(addr).await {
        Ok(_) => {}
        Err(e) => panic!("Error while serving address {addr}: {e}"),
    };
}

pub struct Alpha<L, S> {
    // Mutexes are for interior mutability
    logs_rx: Mutex<Option<UnboundedReceiver<LogItem>>>,
    logs_tx: UnboundedSender<LogItem>,
    stopped_tx: Sender<(StopReason, String)>,
    provisioner_address: Endpoint,
    kill_tx: Mutex<Option<oneshot::Sender<String>>>,
    storage_manager: Arc<dyn StorageManager>,
    loader: Mutex<Option<L>>,
    service: Mutex<Option<S>>,
    env: Environment,
}

impl<L, S> Alpha<L, S> {
    pub fn new(
        provisioner_address: Endpoint,
        loader: L,
        storage_manager: Arc<dyn StorageManager>,
        env: Environment,
    ) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        let (stopped_tx, _stopped_rx) = broadcast::channel(10);

        Self {
            logs_rx: Mutex::new(Some(rx)),
            logs_tx: tx,
            stopped_tx,
            kill_tx: Mutex::new(None),
            provisioner_address,
            storage_manager,
            loader: Mutex::new(Some(loader)),
            service: Mutex::new(None),
            env,
        }
    }
}

#[async_trait]
pub trait Loader<Fac>
where
    Fac: Factory,
{
    type Service: Service;

    async fn load(
        self,
        factory: Fac,
        resource_tracker: ResourceTracker,
        logger: Logger,
    ) -> Result<Self::Service, shuttle_service::Error>;
}

#[async_trait]
impl<F, O, Fac, S> Loader<Fac> for F
where
    F: FnOnce(Fac, ResourceTracker, Logger) -> O + Send,
    O: Future<Output = Result<S, shuttle_service::Error>> + Send,
    Fac: Factory + 'static,
    S: Service,
{
    type Service = S;

    async fn load(
        self,
        factory: Fac,
        resource_tracker: ResourceTracker,
        logger: Logger,
    ) -> Result<Self::Service, shuttle_service::Error> {
        (self)(factory, resource_tracker, logger).await
    }
}

#[async_trait]
impl<L, S> Runtime for Alpha<L, S>
where
    L: Loader<ProvisionerFactory, Service = S> + Send + 'static,
    S: Service + Send + 'static,
{
    async fn load(&self, request: Request<LoadRequest>) -> Result<Response<LoadResponse>, Status> {
        let claim = request.extensions().get::<Claim>().map(Clone::clone);

        let LoadRequest {
            path,
            resources,
            secrets,
            service_name,
        } = request.into_inner();
        trace!(path, "loading alpha project");

        let secrets = BTreeMap::from_iter(secrets.into_iter());

        let channel = self
            .provisioner_address
            .clone()
            .connect()
            .await
            .context("failed to connect to provisioner")
            .map_err(|err| Status::internal(err.to_string()))?;
        let channel = ServiceBuilder::new()
            .layer(ClaimLayer)
            .layer(InjectPropagationLayer)
            .service(channel);

        let provisioner_client = ProvisionerClient::new(channel);

        let service_name = ServiceName::from_str(service_name.as_str())
            .map_err(|err| Status::from_error(Box::new(err)))?;

        let past_resources = resources
            .into_iter()
            .map(resource::Response::from_bytes)
            .collect();
        let new_resources = Arc::new(Mutex::new(Vec::new()));
        let resource_tracker = ResourceTracker::new(past_resources, new_resources.clone());

        let factory = ProvisionerFactory::new(
            provisioner_client,
            service_name,
            secrets,
            self.storage_manager.clone(),
            self.env,
            claim,
        );
        trace!("got factory");

        let logs_tx = self.logs_tx.clone();
        let logger = Logger::new(logs_tx);

        let loader = self.loader.lock().unwrap().deref_mut().take().unwrap();

        let service = match tokio::spawn(loader.load(factory, resource_tracker, logger)).await {
            Ok(res) => match res {
                Ok(service) => service,
                Err(error) => {
                    error!(%error, "loading service failed");

                    let message = LoadResponse {
                        success: false,
                        message: error.to_string(),
                        resources: new_resources
                            .lock()
                            .expect("to get lock no new resources")
                            .iter()
                            .map(resource::Response::to_bytes)
                            .collect(),
                    };
                    return Ok(Response::new(message));
                }
            },
            Err(error) => {
                let resources = new_resources
                    .lock()
                    .expect("to get lock no new resources")
                    .iter()
                    .map(resource::Response::to_bytes)
                    .collect();

                if error.is_panic() {
                    let panic = error.into_panic();
                    let msg = match panic.downcast_ref::<String>() {
                        Some(msg) => msg.to_string(),
                        None => match panic.downcast_ref::<&str>() {
                            Some(msg) => msg.to_string(),
                            None => "<no panic message>".to_string(),
                        },
                    };

                    error!(error = msg, "loading service panicked");

                    let message = LoadResponse {
                        success: false,
                        message: msg,
                        resources,
                    };
                    return Ok(Response::new(message));
                } else {
                    error!(%error, "loading service crashed");
                    let message = LoadResponse {
                        success: false,
                        message: error.to_string(),
                        resources,
                    };
                    return Ok(Response::new(message));
                }
            }
        };

        *self.service.lock().unwrap() = Some(service);

        let message = LoadResponse {
            success: true,
            message: String::new(),
            resources: new_resources
                .lock()
                .expect("to get lock no new resources")
                .iter()
                .map(resource::Response::to_bytes)
                .collect(),
        };
        Ok(Response::new(message))
    }

    async fn start(
        &self,
        request: Request<StartRequest>,
    ) -> Result<Response<StartResponse>, Status> {
        trace!("alpha starting");
        let service = self.service.lock().unwrap().deref_mut().take();
        let service = service.unwrap();

        let StartRequest { ip, .. } = request.into_inner();
        let service_address = SocketAddr::from_str(&ip)
            .context("invalid socket address")
            .map_err(|err| Status::invalid_argument(err.to_string()))?;

        let _logs_tx = self.logs_tx.clone();

        trace!(%service_address, "starting");

        let (kill_tx, kill_rx) = tokio::sync::oneshot::channel();
        *self.kill_tx.lock().unwrap() = Some(kill_tx);

        let stopped_tx = self.stopped_tx.clone();

        let handle = tokio::runtime::Handle::current();

        // start service as a background task with a kill receiver
        tokio::spawn(async move {
            let mut background = handle.spawn(service.bind(service_address));

            tokio::select! {
                res = &mut background => {
                    match res {
                        Ok(_) => {
                            info!("service stopped all on its own");
                            stopped_tx.send((StopReason::End, String::new())).unwrap();
                        },
                        Err(error) => {
                            if error.is_panic() {
                                let panic = error.into_panic();
                                let msg = match panic.downcast_ref::<String>() {
                                    Some(msg) => msg.to_string(),
                                    None => match panic.downcast_ref::<&str>() {
                                        Some(msg) => msg.to_string(),
                                        None => "<no panic message>".to_string(),
                                    },
                                };

                                error!(error = msg, "service panicked");

                                stopped_tx
                                    .send((StopReason::Crash, msg))
                                    .unwrap();
                            } else {
                                error!(%error, "service crashed");
                                stopped_tx
                                    .send((StopReason::Crash, error.to_string()))
                                    .unwrap();
                            }
                        },
                    }
                },
                message = kill_rx => {
                    match message {
                        Ok(_) => {
                            stopped_tx.send((StopReason::Request, String::new())).unwrap();
                        }
                        Err(_) => trace!("the sender dropped")
                    };

                    info!("will now abort the service");
                    background.abort();
                    background.await.unwrap().expect("to stop service");
                }
            }
        });

        let message = StartResponse { success: true };

        Ok(Response::new(message))
    }

    async fn stop(&self, _request: Request<StopRequest>) -> Result<Response<StopResponse>, Status> {
        let kill_tx = self.kill_tx.lock().unwrap().deref_mut().take();

        if let Some(kill_tx) = kill_tx {
            if kill_tx.send("stopping deployment".to_owned()).is_err() {
                error!("the receiver dropped");
                return Err(Status::internal("failed to stop deployment"));
            }

            Ok(Response::new(StopResponse { success: true }))
        } else {
            warn!("failed to stop deployment");

            Ok(tonic::Response::new(StopResponse { success: false }))
        }
    }

    type SubscribeStopStream = ReceiverStream<Result<SubscribeStopResponse, Status>>;

    async fn subscribe_stop(
        &self,
        _request: Request<SubscribeStopRequest>,
    ) -> Result<Response<Self::SubscribeStopStream>, Status> {
        let mut stopped_rx = self.stopped_tx.subscribe();
        let (tx, rx) = mpsc::channel(1);

        // Move the stop channel into a stream to be returned
        tokio::spawn(async move {
            while let Ok((reason, message)) = stopped_rx.recv().await {
                tx.send(Ok(SubscribeStopResponse {
                    reason: reason as i32,
                    message,
                }))
                .await
                .unwrap();
            }
        });

        Ok(Response::new(ReceiverStream::new(rx)))
    }

    type SubscribeLogsStream = ReceiverStream<Result<runtime::LogItem, Status>>;

    async fn subscribe_logs(
        &self,
        _request: Request<SubscribeLogsRequest>,
    ) -> Result<Response<Self::SubscribeLogsStream>, Status> {
        let logs_rx = self.logs_rx.lock().unwrap().deref_mut().take();

        if let Some(mut logs_rx) = logs_rx {
            let (tx, rx) = mpsc::channel(1);

            // Move logger items into stream to be returned
            tokio::spawn(async move {
                while let Some(log) = logs_rx.recv().await {
                    tx.send(Ok(log)).await.expect("to send log");
                }
            });

            Ok(Response::new(ReceiverStream::new(rx)))
        } else {
            Err(Status::internal("logs have already been subscribed to"))
        }
    }
}
