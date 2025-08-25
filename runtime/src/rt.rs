use std::{
    collections::BTreeMap,
    iter::FromIterator,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    process::exit,
    sync::Arc,
};

use anyhow::Context;
use http_body_util::Empty;
use hyper::{body::Bytes, server::conn::http1, service::service_fn, Response};
use hyper_util::rt::TokioIo;
use shuttle_api_client::ShuttleApiClient;
use shuttle_common::{
    models::resource::{ResourceInput, ResourceState, ResourceType},
    secrets::Secret,
};
use shuttle_service::{Environment, ResourceFactory, Service};
use tokio::{net::TcpListener, sync::RwLock};
use tracing::{debug, info, trace};

use crate::__internals::{Loader, Runner};

struct RuntimeEnvVars {
    /// Are we running in a Shuttle deployment?
    shuttle: bool,
    project_id: String,
    project_name: String,
    env: Environment,
    /// Address to open service on
    ip: IpAddr,
    /// Port to open service on
    port: u16,
    /// Optional port to open health check on
    healthz_port: Option<u16>,
    /// Where to reach the required Shuttle API endpoints (mainly for provisioning)
    api_url: String,
    /// Key for the API calls (if relevant)
    api_key: Option<String>,
}

impl RuntimeEnvVars {
    /// Uses primitive parsing instead of clap for reduced dependency weight.
    /// # Panics
    /// if any required arg is missing or does not parse
    fn parse() -> Self {
        Self {
            shuttle: std::env::var("SHUTTLE").is_ok(),
            project_id: std::env::var("SHUTTLE_PROJECT_ID").expect("project id env var"),
            project_name: std::env::var("SHUTTLE_PROJECT_NAME").expect("project name env var"),
            env: std::env::var("SHUTTLE_ENV")
                .expect("shuttle environment env var")
                .parse()
                .expect("invalid shuttle environment"),
            ip: std::env::var("SHUTTLE_RUNTIME_IP")
                .expect("runtime ip env var")
                .parse()
                .expect("invalid ip"),
            port: std::env::var("SHUTTLE_RUNTIME_PORT")
                .expect("runtime port env var")
                .parse()
                .expect("invalid port"),
            healthz_port: std::env::var("SHUTTLE_HEALTHZ_PORT")
                .map(|s| s.parse().expect("invalid healthz port"))
                .ok(),
            api_url: std::env::var("SHUTTLE_API").expect("api url env var"),
            api_key: std::env::var("SHUTTLE_API_KEY").ok(),
        }
    }
}

// uses non-standard exit codes for each scenario to help track down exit reasons
pub async fn start(loader: impl Loader + Send + 'static, runner: impl Runner + Send + 'static) {
    debug!("Parsing environment variables");
    let RuntimeEnvVars {
        shuttle,
        project_id,
        project_name,
        env,
        ip,
        port,
        healthz_port,
        api_url,
        api_key,
    } = RuntimeEnvVars::parse();

    let service_addr = SocketAddr::new(ip, port);
    let client = ShuttleApiClient::new(api_url, api_key, None, None);

    // Shared state for the service (will be set after resource initialization)
    let service_state: Arc<RwLock<Option<Arc<dyn Service>>>> = Arc::new(RwLock::new(None));

    // start a health check server if requested (before provisioning)
    if let Some(healthz_port) = healthz_port {
        trace!("Starting health check server on port {healthz_port}");
        let addr = SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), healthz_port);
        let service_state_clone = service_state.clone();
        
        tokio::spawn(async move {
            // light hyper server
            let Ok(listener) = TcpListener::bind(&addr).await else {
                eprintln!("ERROR: Failed to bind to health check port");
                exit(201);
            };

            loop {
                let Ok((stream, _)) = listener.accept().await else {
                    eprintln!("ERROR: Health check listener error");
                    exit(202);
                };
                let io = TokioIo::new(stream);
                let service_state_ref = service_state_clone.clone();

                tokio::spawn(async move {
                    if let Err(err) = http1::Builder::new()
                        .serve_connection(
                            io,
                            service_fn(move |_req| {
                                let service_state = service_state_ref.clone();
                                async move {
                                    trace!("Received health check");
                                    
                                    // Try to get the service and call its health check
                                    let service_opt = service_state.read().await.clone();
                                    
                                    match service_opt {
                                        Some(service) => {
                                            // Service is available - call its health check
                                            match service.health_check().await {
                                                Ok(()) => {
                                                    trace!("Service health check passed");
                                                    Result::<Response<Empty<Bytes>>, hyper::Error>::Ok(
                                                        Response::new(Empty::new())
                                                    )
                                                }
                                                Err(e) => {
                                                    trace!("Service health check failed: {e}");
                                                    let mut response = Response::new(Empty::new());
                                                    *response.status_mut() = hyper::StatusCode::INTERNAL_SERVER_ERROR;
                                                    Result::<Response<Empty<Bytes>>, hyper::Error>::Ok(response)
                                                }
                                            }
                                        }
                                        None => {
                                            // Service not yet available - return basic health check
                                            trace!("Service not yet initialized, returning basic health check");
                                            Result::<Response<Empty<Bytes>>, hyper::Error>::Ok(
                                                Response::new(Empty::new())
                                            )
                                        }
                                    }
                                }
                            }),
                        )
                        .await
                    {
                        eprintln!("ERROR: Health check error: {err}");
                        exit(200);
                    }
                });
            }
        });
    }

    //
    // LOADING / PROVISIONING PHASE
    //
    info!("Loading resources");

    trace!("Getting secrets");
    let secrets: BTreeMap<String, String> =
        match client.get_secrets(&project_id).await.and_then(|r| {
            serde_json::from_value(r.into_inner().output).context("failed to deserialize secrets")
        }) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("ERROR: Runtime Secret Loading phase failed: {e}");
                exit(101);
            }
        };

    // Sort secrets by key
    let secrets = BTreeMap::from_iter(secrets.into_iter().map(|(k, v)| (k, Secret::new(v))));

    // TODO: rework `ResourceFactory`
    let factory = ResourceFactory::new(project_name, secrets.clone(), env);
    let mut resources = match loader.load(factory).await {
        Ok(r) => r,
        Err(e) => {
            eprintln!("ERROR: Runtime Loader phase failed: {e}");
            exit(111);
        }
    };

    // Fail early if any byte vec is invalid json
    let values = match resources
        .iter()
        .map(|bytes| {
            serde_json::from_slice::<ResourceInput>(bytes).context("deserializing resource input")
        })
        .collect::<anyhow::Result<Vec<_>>>()
    {
        Ok(v) => v,
        Err(e) => {
            eprintln!("ERROR: Runtime Provisioning phase failed: {e}");
            exit(121);
        }
    };

    for (bytes, shuttle_resource) in resources
        .iter_mut()
        .zip(values)
        // ignore non-Shuttle resource items
        .filter_map(|(bytes, value)| match value {
            ResourceInput::Shuttle(shuttle_resource) => Some((bytes, shuttle_resource)),
            ResourceInput::Custom(_) => None,
        })
    {
        // Secrets don't need to be requested here since we already got them above.
        if shuttle_resource.r#type == ResourceType::Secrets {
            *bytes = serde_json::to_vec(&secrets).expect("to serialize struct");
            continue;
        }

        info!("Provisioning {:?}", shuttle_resource.r#type);
        loop {
            trace!("Checking state of {:?}", shuttle_resource.r#type);
            match client
                .provision_resource(&project_id, shuttle_resource.clone())
                .await
                .map(|r| r.into_inner())
            {
                Ok(res) => {
                    trace!("Got response {:?}", res);
                    match res.state {
                        ResourceState::Provisioning | ResourceState::Authorizing => {
                            tokio::time::sleep(tokio::time::Duration::from_millis(2000)).await;
                        }
                        ResourceState::Ready => {
                            *bytes = serde_json::to_vec(&res.output).expect("to serialize struct");
                            break;
                        }
                        bad_state => {
                            eprintln!(
                                "ERROR: Runtime Provisioning phase failed: Received {:?} resource with state '{}'.",
                                shuttle_resource.r#type,
                                bad_state
                            );
                            exit(132);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("ERROR: Runtime Provisioning phase failed: {e}");
                    exit(131);
                }
            };
        }
    }

    // TODO?: call API to say running state is being entered

    if shuttle {
        trace!("Sending sidecar shutdown request");
        // Tell sidecar to shut down.
        // Ignore error, since the endpoint does not send a response.
        let _ = client.client.get("/__shuttle/shutdown").send().await;
    }

    //
    // RESOURCE INIT PHASE
    //

    let service = match runner.run(resources).await {
        Ok(s) => Arc::new(s),
        Err(e) => {
            eprintln!("ERROR: Runtime Resource Initialization phase failed: {e}");
            exit(151);
        }
    };

    // Update the shared service state for health checks
    {
        let mut state = service_state.write().await;
        *state = Some(service.clone());
    }

    // Clone for shutdown
    let service_for_shutdown = service.clone();



    //
    // RUNNING PHASE
    //
    info!("Starting service");

    // Extract the service for binding (this consumes the Arc)
    let service_for_bind = Arc::try_unwrap(service).map_err(|_| {
        eprintln!("ERROR: Failed to unwrap service Arc for binding");
        exit(152);
    }).unwrap_or_else(|_| exit(152));
    
    let service_bind = service_for_bind.bind(service_addr);

    #[cfg(target_family = "unix")]
    let interrupted = {
        let mut sigterm_notif =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                .expect("Can not get the SIGTERM signal receptor");
        let mut sigint_notif =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())
                .expect("Can not get the SIGINT signal receptor");
        tokio::select! {
            res = service_bind => {
                if let Err(e) = res {
                    tracing::error!("Service encountered an error in `bind`: {e}");
                    exit(1);
                }
                tracing::warn!("Service terminated on its own. Shutting down the runtime...");
                false
            }
            _ = sigterm_notif.recv() => {
                tracing::warn!("Received SIGTERM. Shutting down the runtime...");
                true
            },
            _ = sigint_notif.recv() => {
                tracing::warn!("Received SIGINT. Shutting down the runtime...");
                true
            }
        }
    };
    #[cfg(target_family = "windows")]
    let interrupted = {
        let mut ctrl_break_notif = tokio::signal::windows::ctrl_break()
            .expect("Can not get the CtrlBreak signal receptor");
        let mut ctrl_c_notif =
            tokio::signal::windows::ctrl_c().expect("Can not get the CtrlC signal receptor");
        let mut ctrl_close_notif = tokio::signal::windows::ctrl_close()
            .expect("Can not get the CtrlClose signal receptor");
        let mut ctrl_logoff_notif = tokio::signal::windows::ctrl_logoff()
            .expect("Can not get the CtrlLogoff signal receptor");
        let mut ctrl_shutdown_notif = tokio::signal::windows::ctrl_shutdown()
            .expect("Can not get the CtrlShutdown signal receptor");
        tokio::select! {
            res = service_bind => {
                if let Err(e) = res {
                    tracing::error!("Service encountered an error in `bind`: {e}");
                    exit(1);
                }
                tracing::warn!("Service terminated on its own. Shutting down the runtime...");
                false
            }
            _ = ctrl_break_notif.recv() => {
                tracing::warn!("Received ctrl-break. Shutting down the runtime...");
                true
            },
            _ = ctrl_c_notif.recv() => {
                tracing::warn!("Received ctrl-c. Shutting down the runtime...");
                true
            },
            _ = ctrl_close_notif.recv() => {
                tracing::warn!("Received ctrl-close. Shutting down the runtime...");
                true
            },
            _ = ctrl_logoff_notif.recv() => {
                tracing::warn!("Received ctrl-logoff. Shutting down the runtime...");
                true
            },
            _ = ctrl_shutdown_notif.recv() => {
                tracing::warn!("Received ctrl-shutdown. Shutting down the runtime...");
                true
            }
        }
    };

    if interrupted {
        trace!("Calling service shutdown hook");
        if let Err(e) = service_for_shutdown.shutdown().await {
            tracing::error!("Service shutdown hook failed: {e}");
        }
        trace!("Service shutdown completed");
        exit(10);
    }
}
