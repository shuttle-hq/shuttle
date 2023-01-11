use std::{
    collections::HashMap,
    net::{Ipv4Addr, SocketAddr},
    path::PathBuf,
    str::FromStr,
};

use async_trait::async_trait;
use opentelemetry::global;
use portpicker::pick_unused_port;
use shuttle_common::project::ProjectName as ServiceName;
use shuttle_common::storage_manager::ArtifactsStorageManager;
use shuttle_proto::runtime::{runtime_client::RuntimeClient, LoadRequest, StartRequest};

use tokio::task::JoinError;
use tonic::transport::Channel;
use tracing::{debug_span, error, info, instrument, trace, Instrument};
use tracing_opentelemetry::OpenTelemetrySpanExt;
use uuid::Uuid;

use super::{KillReceiver, KillSender, RunReceiver, State};
use crate::{
    error::{Error, Result},
    persistence::SecretGetter,
};

/// Run a task which takes runnable deploys from a channel and starts them up on our runtime
/// A deploy is killed when it receives a signal from the kill channel
pub async fn task(
    mut recv: RunReceiver,
    runtime_client: RuntimeClient<Channel>,
    kill_send: KillSender,
    active_deployment_getter: impl ActiveDeploymentsGetter,
    secret_getter: impl SecretGetter,
    storage_manager: ArtifactsStorageManager,
) {
    info!("Run task started");

    while let Some(built) = recv.recv().await {
        let id = built.id;

        info!("Built deployment at the front of run queue: {id}");

        let kill_send = kill_send.clone();
        let kill_recv = kill_send.subscribe();
        let secret_getter = secret_getter.clone();
        let storage_manager = storage_manager.clone();

        let _service_name = match ServiceName::from_str(&built.service_name) {
            Ok(name) => name,
            Err(err) => {
                start_crashed_cleanup(&id, err);
                continue;
            }
        };

        let old_deployments_killer = kill_old_deployments(
            built.service_id,
            id,
            active_deployment_getter.clone(),
            kill_send,
        );
        let cleanup = move |result: std::result::Result<
            std::result::Result<(), shuttle_service::Error>,
            JoinError,
        >| match result {
            Ok(inner) => match inner {
                Ok(()) => completed_cleanup(&id),
                Err(err) => crashed_cleanup(&id, err),
            },
            Err(err) if err.is_cancelled() => stopped_cleanup(&id),
            Err(err) => start_crashed_cleanup(&id, err),
        };
        let runtime_client = runtime_client.clone();

        tokio::spawn(async move {
            let parent_cx = global::get_text_map_propagator(|propagator| {
                propagator.extract(&built.tracing_context)
            });
            let span = debug_span!("runner");
            span.set_parent(parent_cx);

            async move {
                if let Err(err) = built
                    .handle(
                        storage_manager,
                        secret_getter,
                        runtime_client,
                        kill_recv,
                        old_deployments_killer,
                        cleanup,
                    )
                    .await
                {
                    start_crashed_cleanup(&id, err)
                }

                info!("deployment done");
            }
            .instrument(span)
            .await
        });
    }
}

#[instrument(skip(active_deployment_getter, kill_send))]
async fn kill_old_deployments(
    service_id: Uuid,
    deployment_id: Uuid,
    active_deployment_getter: impl ActiveDeploymentsGetter,
    kill_send: KillSender,
) -> Result<()> {
    for old_id in active_deployment_getter
        .clone()
        .get_active_deployments(&service_id)
        .await
        .map_err(|e| Error::OldCleanup(Box::new(e)))?
        .into_iter()
        .filter(|old_id| old_id != &deployment_id)
    {
        trace!(%old_id, "stopping old deployment");
        kill_send
            .send(old_id)
            .map_err(|e| Error::OldCleanup(Box::new(e)))?;
    }

    Ok(())
}

#[instrument(skip(_id), fields(id = %_id, state = %State::Completed))]
fn completed_cleanup(_id: &Uuid) {
    info!("service finished all on its own");
}

#[instrument(skip(_id), fields(id = %_id, state = %State::Stopped))]
fn stopped_cleanup(_id: &Uuid) {
    info!("service was stopped by the user");
}

#[instrument(skip(_id), fields(id = %_id, state = %State::Crashed))]
fn crashed_cleanup(_id: &Uuid, error: impl std::error::Error + 'static) {
    error!(
        error = &error as &dyn std::error::Error,
        "service encountered an error"
    );
}

#[instrument(skip(_id), fields(id = %_id, state = %State::Crashed))]
fn start_crashed_cleanup(_id: &Uuid, error: impl std::error::Error + 'static) {
    error!(
        error = &error as &dyn std::error::Error,
        "service startup encountered an error"
    );
}

#[async_trait]
pub trait ActiveDeploymentsGetter: Clone + Send + Sync + 'static {
    type Err: std::error::Error + Send;

    async fn get_active_deployments(
        &self,
        service_id: &Uuid,
    ) -> std::result::Result<Vec<Uuid>, Self::Err>;
}

#[derive(Clone, Debug)]
pub struct Built {
    pub id: Uuid,
    pub service_name: String,
    pub service_id: Uuid,
    pub tracing_context: HashMap<String, String>,
    pub is_next: bool,
}

impl Built {
    #[instrument(skip(self, storage_manager, secret_getter, runtime_client, kill_recv, kill_old_deployments, cleanup), fields(id = %self.id, state = %State::Loading))]
    #[allow(clippy::too_many_arguments)]
    async fn handle(
        self,
        storage_manager: ArtifactsStorageManager,
        secret_getter: impl SecretGetter,
        runtime_client: RuntimeClient<Channel>,
        kill_recv: KillReceiver,
        kill_old_deployments: impl futures::Future<Output = Result<()>>,
        cleanup: impl FnOnce(std::result::Result<std::result::Result<(), shuttle_service::Error>, JoinError>)
            + Send
            + 'static,
    ) -> Result<()> {
        let so_path = storage_manager.deployment_library_path(&self.id)?;

        let port = match pick_unused_port() {
            Some(port) => port,
            None => {
                return Err(Error::PrepareRun(
                    "could not find a free port to deploy service on".to_string(),
                ))
            }
        };

        let address = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), port);

        kill_old_deployments.await?;

        info!("got handle for deployment");
        // Execute loaded service
        load(
            self.service_name.clone(),
            self.service_id,
            so_path,
            secret_getter,
            runtime_client.clone(),
        )
        .await;
        tokio::spawn(run(
            self.id,
            self.service_name,
            runtime_client,
            address,
            kill_recv,
            cleanup,
        ));

        Ok(())
    }
}

async fn load(
    service_name: String,
    service_id: Uuid,
    so_path: PathBuf,
    secret_getter: impl SecretGetter,
    mut runtime_client: RuntimeClient<Channel>,
) {
    info!(
        "loading project from: {}",
        so_path.clone().into_os_string().into_string().unwrap()
    );

    let secrets = secret_getter
        .get_secrets(&service_id)
        .await
        .unwrap()
        .into_iter()
        .map(|secret| (secret.key, secret.value));
    let secrets = HashMap::from_iter(secrets);

    let load_request = tonic::Request::new(LoadRequest {
        path: so_path.into_os_string().into_string().unwrap(),
        service_name: service_name.clone(),
        secrets,
    });
    info!("loading service");
    let response = runtime_client.load(load_request).await;

    if let Err(e) = response {
        info!("failed to load service: {}", e);
    }
}

#[instrument(skip(runtime_client, _kill_recv, _cleanup), fields(state = %State::Running))]
async fn run(
    id: Uuid,
    service_name: String,
    mut runtime_client: RuntimeClient<Channel>,
    address: SocketAddr,
    _kill_recv: KillReceiver,
    _cleanup: impl FnOnce(std::result::Result<std::result::Result<(), shuttle_service::Error>, JoinError>)
        + Send
        + 'static,
) {
    let start_request = tonic::Request::new(StartRequest {
        deployment_id: id.as_bytes().to_vec(),
        service_name,
        port: address.port() as u32,
    });

    info!("starting service");
    let response = runtime_client.start(start_request).await.unwrap();

    info!(response = ?response.into_inner(),  "client response: ");
}

#[cfg(test)]
mod tests {
    use std::{path::PathBuf, process::Command, time::Duration};

    use async_trait::async_trait;
    use shuttle_common::storage_manager::ArtifactsStorageManager;
    use shuttle_proto::runtime::runtime_client::RuntimeClient;
    use tempdir::TempDir;
    use tokio::{
        sync::{broadcast, oneshot},
        task::JoinError,
        time::sleep,
    };
    use tonic::transport::Channel;
    use uuid::Uuid;

    use crate::{
        error::Error,
        persistence::{Secret, SecretGetter},
    };

    use super::Built;

    const RESOURCES_PATH: &str = "tests/resources";

    fn get_storage_manager() -> ArtifactsStorageManager {
        let tmp_dir = TempDir::new("shuttle_run_test").unwrap();
        let path = tmp_dir.into_path();

        ArtifactsStorageManager::new(path)
    }

    async fn kill_old_deployments() -> crate::error::Result<()> {
        Ok(())
    }

    async fn get_runtime_client() -> RuntimeClient<Channel> {
        RuntimeClient::connect("http://127.0.0.1:6001")
            .await
            .unwrap()
    }

    #[derive(Clone)]
    struct StubSecretGetter;

    #[async_trait]
    impl SecretGetter for StubSecretGetter {
        type Err = std::io::Error;

        async fn get_secrets(&self, _service_id: &Uuid) -> Result<Vec<Secret>, Self::Err> {
            Ok(Default::default())
        }
    }

    fn get_secret_getter() -> StubSecretGetter {
        StubSecretGetter
    }

    // This test uses the kill signal to make sure a service does stop when asked to
    #[tokio::test]
    async fn can_be_killed() {
        let (built, storage_manager) = make_so_and_built("sleep-async");
        let id = built.id;
        let (kill_send, kill_recv) = broadcast::channel(1);
        let (cleanup_send, cleanup_recv) = oneshot::channel();

        let handle_cleanup = |result: std::result::Result<
            std::result::Result<(), shuttle_service::Error>,
            JoinError,
        >| {
            assert!(
                matches!(result, Err(ref join_error) if join_error.is_cancelled()),
                "handle should have been cancelled: {:?}",
                result
            );
            cleanup_send.send(()).unwrap();
        };
        let secret_getter = get_secret_getter();

        built
            .handle(
                storage_manager,
                secret_getter,
                get_runtime_client().await,
                kill_recv,
                kill_old_deployments(),
                handle_cleanup,
            )
            .await
            .unwrap();

        // Give it some time to start up
        sleep(Duration::from_secs(1)).await;

        // Send kill signal
        kill_send.send(id).unwrap();

        tokio::select! {
            _ = sleep(Duration::from_secs(1)) => panic!("cleanup should have been called"),
            Ok(()) = cleanup_recv => {}
        }
    }

    // This test does not use a kill signal to stop the service. Rather the service decided to stop on its own without errors
    #[tokio::test]
    async fn self_stop() {
        let (built, storage_manager) = make_so_and_built("sleep-async");
        let (_kill_send, kill_recv) = broadcast::channel(1);
        let (cleanup_send, cleanup_recv) = oneshot::channel();

        let handle_cleanup = |result: std::result::Result<
            std::result::Result<(), shuttle_service::Error>,
            JoinError,
        >| {
            let result = result.unwrap();
            assert!(
                result.is_ok(),
                "did not expect error from self stopping service: {}",
                result.unwrap_err()
            );
            cleanup_send.send(()).unwrap();
        };
        let secret_getter = get_secret_getter();

        built
            .handle(
                storage_manager,
                secret_getter,
                get_runtime_client().await,
                kill_recv,
                kill_old_deployments(),
                handle_cleanup,
            )
            .await
            .unwrap();

        tokio::select! {
            _ = sleep(Duration::from_secs(5)) => panic!("cleanup should have been called as service stopped on its own"),
            Ok(()) = cleanup_recv => {},
        }
    }

    // Test for panics in Service::bind
    #[tokio::test]
    async fn panic_in_bind() {
        let (built, storage_manager) = make_so_and_built("bind-panic");
        let (_kill_send, kill_recv) = broadcast::channel(1);
        let (cleanup_send, cleanup_recv): (oneshot::Sender<()>, _) = oneshot::channel();

        let handle_cleanup = |result: std::result::Result<
            std::result::Result<(), shuttle_service::Error>,
            JoinError,
        >| {
            let result = result.unwrap();
            assert!(
                matches!(result, Err(shuttle_service::Error::BindPanic(ref msg)) if msg == "panic in bind"),
                "expected inner error from handle: {:?}",
                result
            );
            cleanup_send.send(()).unwrap();
        };
        let secret_getter = get_secret_getter();

        built
            .handle(
                storage_manager,
                secret_getter,
                get_runtime_client().await,
                kill_recv,
                kill_old_deployments(),
                handle_cleanup,
            )
            .await
            .unwrap();

        tokio::select! {
            _ = sleep(Duration::from_secs(5)) => panic!("cleanup should have been called as service handle stopped after panic"),
            Ok(()) = cleanup_recv => {}
        }
    }

    // Test for panics in the main function
    #[tokio::test]
    async fn panic_in_main() {
        let (built, storage_manager) = make_so_and_built("main-panic");
        let (_kill_send, kill_recv) = broadcast::channel(1);

        let handle_cleanup = |_result| panic!("the service shouldn't even start");
        let secret_getter = get_secret_getter();

        let result = built
            .handle(
                storage_manager,
                secret_getter,
                get_runtime_client().await,
                kill_recv,
                kill_old_deployments(),
                handle_cleanup,
            )
            .await;

        assert!(
            matches!(result, Err(Error::Run(shuttle_service::Error::BuildPanic(ref msg))) if msg == "main panic"),
            "expected inner error from main: {:?}",
            result
        );
    }

    #[tokio::test]
    async fn missing_so() {
        let built = Built {
            id: Uuid::new_v4(),
            service_name: "test".to_string(),
            service_id: Uuid::new_v4(),
            tracing_context: Default::default(),
            is_next: false,
        };
        let (_kill_send, kill_recv) = broadcast::channel(1);

        let handle_cleanup = |_result| panic!("no service means no cleanup");
        let secret_getter = get_secret_getter();
        let storage_manager = get_storage_manager();

        let result = built
            .handle(
                storage_manager,
                secret_getter,
                get_runtime_client().await,
                kill_recv,
                kill_old_deployments(),
                handle_cleanup,
            )
            .await;

        assert!(
            matches!(
                result,
                Err(Error::Load(shuttle_service::loader::LoaderError::Load(_)))
            ),
            "expected missing 'so' error: {:?}",
            result
        );
    }

    fn make_so_and_built(crate_name: &str) -> (Built, ArtifactsStorageManager) {
        let crate_dir: PathBuf = [RESOURCES_PATH, crate_name].iter().collect();

        Command::new("cargo")
            .args(["build", "--release"])
            .current_dir(&crate_dir)
            .spawn()
            .unwrap()
            .wait()
            .unwrap();

        let dashes_replaced = crate_name.replace('-', "_");

        let lib_name = if cfg!(target_os = "windows") {
            format!("{}.dll", dashes_replaced)
        } else {
            format!("lib{}.so", dashes_replaced)
        };

        let id = Uuid::new_v4();
        let so_path = crate_dir.join("target/release").join(lib_name);
        let storage_manager = get_storage_manager();
        let new_so_path = storage_manager.deployment_library_path(&id).unwrap();

        std::fs::copy(so_path, new_so_path).unwrap();

        (
            Built {
                id,
                service_name: crate_name.to_string(),
                service_id: Uuid::new_v4(),
                tracing_context: Default::default(),
                is_next: false,
            },
            storage_manager,
        )
    }
}
