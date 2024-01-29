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
        trace::ExtractPropagationLayer,
    },
    claims::{Claim, ClaimLayer, InjectPropagationLayer},
    resource,
    secrets::Secret,
};
use shuttle_proto::{
    provisioner::provisioner_client::ProvisionerClient,
    runtime::{
        runtime_server::{Runtime, RuntimeServer},
        LoadRequest, LoadResponse, StartRequest, StartResponse, StopReason, StopRequest,
        StopResponse, SubscribeStopRequest, SubscribeStopResponse,
    },
};
use shuttle_service::{Environment, Factory, Service};
use tokio::sync::{
    broadcast::{self, Sender},
    mpsc, oneshot,
};
use tokio_stream::wrappers::ReceiverStream;
use tonic::{
    transport::{Endpoint, Server},
    Request, Response, Status,
};
use tower::ServiceBuilder;

use crate::__internals::{print_version, ProvisionerFactory, ResourceTracker};

use self::args::Args;

mod args;

pub async fn start(loader: impl Loader<ProvisionerFactory> + Send + 'static) {
    // `--version` overrides any other arguments.
    if std::env::args().any(|arg| arg == "--version") {
        print_version();
        return;
    }

    let args = match Args::parse() {
        Ok(args) => args,
        Err(e) => {
            eprintln!("Runtime received malformed or incorrect args, {e}");
            let help_str = "[HINT]: Run shuttle with `cargo shuttle run`";
            let wrapper_str = "-".repeat(help_str.len());
            eprintln!("{wrapper_str}\n{help_str}\n{wrapper_str}");
            return;
        }
    };

    println!(
        "shuttle-runtime executable started (version {})",
        crate::VERSION
    );

    // this is handled after arg parsing to not interfere with --version above
    #[cfg(feature = "setup-tracing")]
    {
        use colored::Colorize;
        use tracing_subscriber::prelude::*;

        colored::control::set_override(true); // always apply color

        tracing_subscriber::registry()
            .with(tracing_subscriber::fmt::layer().without_time())
            .with(
                // let user override RUST_LOG in local run if they want to
                tracing_subscriber::EnvFilter::try_from_default_env()
                    // otherwise use our default
                    .or_else(|_| tracing_subscriber::EnvFilter::try_new("info,shuttle=trace"))
                    .unwrap(),
            )
            .init();

        println!(
            "{}\n\
            {}\n\
            To disable the subscriber and use your own,\n\
            turn off the default features for {}:\n\
            \n\
            {}\n\
            {}",
            "=".repeat(63).yellow(),
            "Shuttle's default tracing subscriber is initialized!"
                .yellow()
                .bold(),
            "shuttle-runtime".italic(),
            r#"shuttle-runtime = { version = "...", default-features = false }"#.italic(),
            "=".repeat(63).yellow(),
        );
    }

    let addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), args.port);

    let provisioner_address = args.provisioner_address;
    let mut server_builder = Server::builder()
        .http2_keepalive_interval(Some(Duration::from_secs(60)))
        .layer(JwtAuthenticationLayer::new(AuthPublicKey::new(
            args.auth_uri,
        )))
        .layer(ExtractPropagationLayer);

    let router = {
        let alpha = Alpha::new(provisioner_address, loader, args.env);

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
    stopped_tx: Sender<(StopReason, String)>,
    provisioner_address: Endpoint,
    kill_tx: Mutex<Option<oneshot::Sender<String>>>,
    loader: Mutex<Option<L>>,
    service: Mutex<Option<S>>,
    env: Environment,
}

impl<L, S> Alpha<L, S> {
    pub fn new(provisioner_address: Endpoint, loader: L, env: Environment) -> Self {
        let (stopped_tx, _stopped_rx) = broadcast::channel(10);

        Self {
            stopped_tx,
            kill_tx: Mutex::new(None),
            provisioner_address,
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
    ) -> Result<Self::Service, shuttle_service::Error>;
}

#[async_trait]
impl<F, O, Fac, S> Loader<Fac> for F
where
    F: FnOnce(Fac, ResourceTracker) -> O + Send,
    O: Future<Output = Result<S, shuttle_service::Error>> + Send,
    Fac: Factory + 'static,
    S: Service,
{
    type Service = S;

    async fn load(
        self,
        factory: Fac,
        resource_tracker: ResourceTracker,
    ) -> Result<Self::Service, shuttle_service::Error> {
        (self)(factory, resource_tracker).await
    }
}

#[async_trait]
impl<L, S> Runtime for Alpha<L, S>
where
    L: Loader<ProvisionerFactory, Service = S> + Send + 'static,
    S: Service + Send + 'static,
{
    async fn load(&self, request: Request<LoadRequest>) -> Result<Response<LoadResponse>, Status> {
        let claim = request.extensions().get::<Claim>().cloned();

        let LoadRequest {
            path,
            resources,
            secrets,
            service_name,
        } = request.into_inner();
        println!("loading alpha service at {path}");

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

        // TODO: merge new & old secrets

        let past_resources = resources
            .into_iter()
            .map(resource::Response::from_bytes)
            .collect();
        let new_resources = Arc::new(Mutex::new(Vec::new()));
        let resource_tracker = ResourceTracker::new(past_resources, new_resources.clone());

        // Sorts secrets by key
        let secrets = BTreeMap::from_iter(secrets.into_iter().map(|(k, v)| (k, Secret::new(v))));

        let factory =
            ProvisionerFactory::new(provisioner_client, service_name, secrets, self.env, claim);

        let loader = self.loader.lock().unwrap().deref_mut().take().unwrap();

        // send to new thread to catch panics
        let service = match tokio::spawn(loader.load(factory, resource_tracker)).await {
            Ok(res) => match res {
                Ok(service) => service,
                Err(error) => {
                    println!("loading service failed: {error:#}");

                    let message = LoadResponse {
                        success: false,
                        message: error.to_string(),
                        resources: new_resources
                            .lock()
                            .expect("to get lock on new resources")
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
                    .expect("to get lock on new resources")
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

                    println!("loading service panicked: {msg}");

                    let message = LoadResponse {
                        success: false,
                        message: msg,
                        resources,
                    };
                    return Ok(Response::new(message));
                } else {
                    println!("loading service crashed: {error:#}");
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
                .expect("to get lock on new resources")
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
        let service = self.service.lock().unwrap().deref_mut().take();
        let service = service.unwrap();

        let StartRequest { ip, .. } = request.into_inner();
        let service_address = SocketAddr::from_str(&ip)
            .context("invalid socket address")
            .map_err(|err| Status::invalid_argument(err.to_string()))?;

        println!("Starting on {service_address}");

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
                            println!("service stopped all on its own");
                            let _ = stopped_tx
                                .send((StopReason::End, String::new()))
                                .map_err(|e| println!("{e}"));
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

                                println!("service panicked: {msg}");

                                let _ = stopped_tx
                                    .send((StopReason::Crash, msg))
                                    .map_err(|e| println!("{e}"));
                            } else {
                                println!("service crashed: {error}");
                                let _ = stopped_tx
                                    .send((StopReason::Crash, error.to_string()))
                                    .map_err(|e| println!("{e}"));
                            }
                        },
                    }
                },
                message = kill_rx => {
                    match message {
                        Ok(_) => {
                            let _ = stopped_tx
                                .send((StopReason::Request, String::new()))
                                .map_err(|e| println!("{e}"));
                        }
                        Err(_) => println!("the kill sender dropped")
                    };

                    println!("will now abort the service");
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
                println!("the kill receiver dropped");
                return Err(Status::internal("failed to stop deployment"));
            }

            Ok(Response::new(StopResponse { success: true }))
        } else {
            println!("failed to stop deployment");

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
}
