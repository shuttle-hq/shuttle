use std::{
    collections::HashMap,
    net::{IpAddr, Ipv4Addr, SocketAddr},
};

use opentelemetry::global;
use shuttle_common::claims::{Claim, ClaimService, InjectPropagation};

use shuttle_proto::runtime::{
    runtime_client::RuntimeClient, LoadRequest, StartRequest, StopReason, SubscribeStopRequest,
    SubscribeStopResponse,
};
use tokio::sync::mpsc;
use tonic::{transport::Channel, Code};
use tracing::{debug, debug_span, error, info, instrument, warn, Instrument};
use tracing_opentelemetry::OpenTelemetrySpanExt;
use ulid::Ulid;

use crate::{
    dal::Dal,
    project::service::{
        state::{
            c_starting::ServiceStarting, f_running::ServiceRunning, g_completed::ServiceCompleted,
            m_destroyed::ServiceDestroyed, m_errored::ServiceErrored, StateVariant,
        },
        ServiceState,
    },
    runtime_manager::RuntimeManager,
};

use crate::dal::DalError;

type RunReceiver = mpsc::Receiver<RunnableDeployment>;

pub const USER_SERVICE_DEFAULT_PORT: u16 = 8080;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Database error")]
    Dal(DalError),
    #[error("Missing IPv4 address in the persistence")]
    MissingIpv4Address,
    #[error("Error occurred when running a deployment: {0}")]
    Send(String),
    #[error("Error at service runtime: {0}")]
    Runtime(anyhow::Error),
    #[error("Error preparing the service runtime: {0}")]
    PrepareRun(String),
    #[error("Encountered IO error: {0}")]
    IoError(std::io::Error),
    #[error("Error during the service load phase: {0}")]
    Load(String),
    #[error("Error during the service run phase: {0}")]
    Start(String),
}

pub type Result<T> = std::result::Result<T, Error>;

/// Run a task which takes runnable deploys from a channel and starts them up on our runtime
/// A deploy is killed when it receives a signal from the kill channel
pub async fn task<D: Dal + Sync + 'static>(
    dal: D,
    mut recv: RunReceiver,
    mut runtime_manager: RuntimeManager,
) {
    info!("Run task started");

    while let Some(runnable) = recv.recv().await {
        info!("Deployment to be run: {}", runnable.deployment_id);
        let runtime_client = runtime_manager
            .runtime_client(
                runnable.service_id,
                runnable
                    .target_ip
                    .expect("to have a target ip set for the runtime client"),
            )
            .await
            .expect("to set up a runtime client against a ready deployment");
        let claim = runnable.claim.clone();
        let runtime_manager = runtime_manager.clone();
        let dal = dal.clone();
        tokio::spawn(async move {
            let parent_cx = global::get_text_map_propagator(|propagator| {
                propagator.extract(&runnable.tracing_context)
            });
            let span = debug_span!("runner");
            span.set_parent(parent_cx);

            let deployment_id = runnable.deployment_id;

            // We are subscribing to the runtime logs emitted during the load phase here.
            if let Err(err) = runtime_manager
                .logs_subscribe(&runnable.service_id)
                .await
                .map_err(|err| Error::PrepareRun(err.to_string()))
            {
                start_crashed_cleanup(&deployment_id, err);
            }

            async move {
                if let Err(err) = runnable.load_and_run(dal, runtime_client, claim).await {
                    start_crashed_cleanup(&deployment_id, err)
                }

                info!("deployment done");
            }
            .instrument(span)
            .await
        });
    }
}

async fn cleanup<D: Dal + Sync + 'static>(
    response: Option<SubscribeStopResponse>,
    deployment_id: &Ulid,
    dal: D,
) {
    debug!(response = ?response,  "stop client response: ");

    if let Some(response) = response {
        match StopReason::from_i32(response.reason).unwrap_or_default() {
            StopReason::Request => stopped_cleanup(deployment_id),
            StopReason::End => completed_cleanup(deployment_id, dal).await.unwrap_or(()),
            StopReason::Crash => crashed_cleanup(
                deployment_id,
                Error::Runtime(anyhow::Error::msg(response.message)),
            ),
        }
    } else {
        crashed_cleanup(
            deployment_id,
            Error::Runtime(anyhow::anyhow!(
                "stop subscribe channel stopped unexpectedly"
            )),
        )
    }
}

#[instrument(skip(deployment_id, dal), fields(deployment_id = %deployment_id, state = %ServiceCompleted::name()))]
async fn completed_cleanup<D: Dal + Sync + 'static>(deployment_id: &Ulid, dal: D) -> Result<()> {
    info!("service was stopped by its own");

    // We're updating the service state. We should expect to have the container end by itself.
    let deployment = dal.deployment(deployment_id).await.map_err(Error::Dal)?;
    let service = dal
        .service(&deployment.service_id)
        .await
        .map_err(Error::Dal)?;
    if let Some(container) = service.state.container() {
        dal.update_service_state(
            service.id,
            ServiceState::Completed(
                ServiceCompleted::from_container(container)
                    .expect("to return a valid completed state"),
            ),
        )
        .await
        .map_err(Error::Dal)?;
    }

    Ok(())
}

#[instrument(skip(_id), fields(id = %_id, state = %ServiceDestroyed::name()))]
fn stopped_cleanup(_id: &Ulid) {
    // TODO: pass over here the necessary information to destroy the container.
    info!("service was stopped by the user");
}

#[instrument(skip(_id), fields(id = %_id, state = %ServiceErrored::name()))]
fn crashed_cleanup(_id: &Ulid, error: impl std::error::Error + 'static) {
    // TODO: pass over here the necessary information to remove any exisiting container.
    error!(
        error = &error as &dyn std::error::Error,
        "service encountered an error"
    );
}

#[instrument(skip(_id), fields(id = %_id, state = %ServiceErrored::name()))]
fn start_crashed_cleanup(_id: &Ulid, error: impl std::error::Error + 'static) {
    // TODO: pass over here the necessary information to remove any exisiting container.
    error!(
        error = &error as &dyn std::error::Error,
        "service startup encountered an error"
    );
}

#[derive(Clone, Debug)]
pub struct RunnableDeployment {
    pub deployment_id: Ulid,
    pub service_name: String,
    pub service_id: Ulid,
    pub tracing_context: HashMap<String, String>,
    // When deployments are reinstated at the deployment startup there is no claim.
    pub claim: Option<Claim>,
    // The target IP of the deployment, which is resolved after the service gets attached to the network.
    pub target_ip: Option<Ipv4Addr>,
    pub is_next: bool,
}

impl RunnableDeployment {
    #[instrument(skip(self, dal, runtime_client, claim), fields(id = %self.deployment_id, state = %ServiceStarting::name()))]
    #[allow(clippy::too_many_arguments)]
    async fn load_and_run<D: Dal + Sync + 'static>(
        self,
        dal: D,
        runtime_client: RuntimeClient<ClaimService<InjectPropagation<Channel>>>,
        claim: Option<Claim>,
    ) -> Result<()> {
        let address = SocketAddr::new(
            IpAddr::V4(self.target_ip.expect("to have a target ip set")),
            USER_SERVICE_DEFAULT_PORT,
        );

        // Execute loaded service
        load(self.service_name.clone(), runtime_client.clone(), claim).await?;

        tokio::spawn(run(self.deployment_id, dal, runtime_client, address));

        Ok(())
    }
}

async fn load(
    service_name: String,
    mut runtime_client: RuntimeClient<ClaimService<InjectPropagation<Channel>>>,
    claim: Option<Claim>,
) -> Result<()> {
    // For alpha this is the path to the users project with an embedded runtime.
    // For shuttle-next this is the path to the compiled .wasm file, which will be
    // used in the load request.
    // TODO: we need pass down the path for shuttle next
    let path = String::new();
    let mut load_request = tonic::Request::new(LoadRequest {
        path,
        service_name,
        resources: Vec::new(),
        secrets: HashMap::new(),
    });

    if let Some(claim) = claim {
        load_request.extensions_mut().insert(claim);
    }

    debug!("loading service");
    let response = runtime_client.load(load_request).await;

    match response {
        Ok(response) => {
            let response = response.into_inner();

            info!(success = %response.success, "loading response");

            if response.success {
                Ok(())
            } else {
                error!(error = %response.message, "failed to load service");
                Err(Error::Load(response.message))
            }
        }
        Err(error) => {
            error!(%error, "failed to load service");
            Err(Error::Load(error.to_string()))
        }
    }
}

#[instrument(skip(runtime_client, dal), fields(id=%id, state = %ServiceRunning::name()))]
async fn run<D: Dal + Sync + 'static>(
    id: Ulid,
    dal: D,
    mut runtime_client: RuntimeClient<ClaimService<InjectPropagation<Channel>>>,
    address: SocketAddr,
) {
    let start_request = tonic::Request::new(StartRequest {
        ip: address.to_string(),
    });

    // Subscribe to stop before starting to catch immediate errors
    let mut stream = runtime_client
        .subscribe_stop(tonic::Request::new(SubscribeStopRequest {}))
        .await
        .unwrap()
        .into_inner();

    info!("starting service");
    let response = runtime_client.start(start_request).await;

    match response {
        Ok(response) => {
            info!(response = ?response.into_inner(),  "start client response: ");

            // Wait for stop reason
            let reason = stream.message().await.expect("message from tonic stream");

            cleanup(reason, &id, dal).await;
        }
        Err(ref status) if status.code() == Code::InvalidArgument => {
            cleanup(
                Some(SubscribeStopResponse {
                    reason: StopReason::Crash as i32,
                    message: status.to_string(),
                }),
                &id,
                dal,
            )
            .await;
        }
        Err(ref status) => {
            start_crashed_cleanup(
                &id,
                Error::Start("runtime failed to start deployment".to_string()),
            );

            error!(%status, "failed to start service");
        }
    }
}
