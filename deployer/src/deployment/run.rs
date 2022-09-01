use std::{
    net::{Ipv4Addr, SocketAddr},
    path::PathBuf,
    str::FromStr,
};

use portpicker::pick_unused_port;
use shuttle_common::project::ProjectName;
use shuttle_service::{
    loader::{LoadedService, Loader},
    Factory,
};
use tokio::task::JoinError;
use tracing::{debug, error, info, instrument};
use uuid::Uuid;

use super::{
    provisioner_factory, queue::LIBS_PATH, runtime_logger, KillReceiver, KillSender, RunReceiver,
    State,
};
use crate::error::{Error, Result};

/// Run a task which takes runnable deploys from a channel and starts them up with a factory provided by the
/// abstract factory and a runtime logger provided by the logger factory
/// A deploy is killed when it receives a signal from the kill channel
pub async fn task(
    mut recv: RunReceiver,
    kill_send: KillSender,
    abstract_factory: impl provisioner_factory::AbstractFactory,
    logger_factory: impl runtime_logger::Factory,
) {
    info!("Run task started");

    while let Some(built) = recv.recv().await {
        let id = built.id;

        info!("Built deployment at the front of run queue: {id}");

        let kill_recv = kill_send.subscribe();

        let port = match pick_unused_port() {
            Some(port) => port,
            None => {
                start_crashed_cleanup(
                    &id,
                    Error::PrepareLoad(
                        "could not find a free port to deploy service on".to_string(),
                    ),
                );
                continue;
            }
        };
        let addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), port);
        let project_name = match ProjectName::from_str(&built.name) {
            Ok(name) => name,
            Err(err) => {
                start_crashed_cleanup(&id, err);
                continue;
            }
        };
        let mut factory = abstract_factory.get_factory(project_name);
        let logger = logger_factory.get_logger(id);
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

        tokio::spawn(async move {
            if let Err(err) = built
                .handle(addr, &mut factory, logger, kill_recv, cleanup)
                .await
            {
                start_crashed_cleanup(&id, err)
            }

            info!("deployment done");
        });
    }
}

#[instrument(fields(id = %_id, state = %State::Completed))]
fn completed_cleanup(_id: &Uuid) {
    info!("service finished all on its own");
}

#[instrument(fields(id = %_id, state = %State::Stopped))]
fn stopped_cleanup(_id: &Uuid) {
    info!("service was stopped by the user");
}

#[instrument(fields(id = %_id, state = %State::Crashed))]
fn crashed_cleanup(_id: &Uuid, err: impl std::error::Error + 'static) {
    error!(
        error = &err as &dyn std::error::Error,
        "service encountered an error"
    );
}

#[instrument(fields(id = %_id, state = %State::Crashed))]
fn start_crashed_cleanup(_id: &Uuid, err: impl std::error::Error + 'static) {
    error!(
        error = &err as &dyn std::error::Error,
        "service startup encountered an error"
    );
}

#[derive(Clone, Debug)]
pub struct Built {
    pub id: Uuid,
    pub name: String,
}

impl Built {
    #[instrument(name = "built_handle", skip(self, factory, logger, cleanup), fields(id = %self.id, state = %State::Running))]
    async fn handle(
        self,
        addr: SocketAddr,
        factory: &mut dyn Factory,
        logger: Box<dyn log::Log>,
        mut kill_recv: KillReceiver,
        cleanup: impl FnOnce(std::result::Result<std::result::Result<(), shuttle_service::Error>, JoinError>)
            + Send
            + 'static,
    ) -> Result<()> {
        let (mut handle, library) = load_deployment(&self.id, addr, factory, logger).await?;

        info!("got handle for deployment");
        // Execute loaded service
        tokio::spawn(async move {
            let result;
            loop {
                tokio::select! {
                     Ok(id) = kill_recv.recv() => {
                         if id == self.id {
                             debug!("deployment '{id}' killed");
                             handle.abort();
                             result = handle.await;
                             break;
                         }
                     }
                     rsl = &mut handle => {
                         result = rsl;
                         break;
                     }
                }
            }

            if let Err(err) = library.close() {
                crashed_cleanup(&self.id, err);
            } else {
                cleanup(result);
            }
        });

        Ok(())
    }
}

#[instrument(skip(id, addr, factory, logger))]
async fn load_deployment(
    id: &Uuid,
    addr: SocketAddr,
    factory: &mut dyn Factory,
    logger: Box<dyn log::Log>,
) -> Result<LoadedService> {
    let so_path = PathBuf::from(LIBS_PATH).join(id.to_string());
    let loader = Loader::from_so_file(so_path)?;

    Ok(loader.load(factory, addr, logger).await?)
}

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeMap,
        net::{Ipv4Addr, SocketAddr},
        path::PathBuf,
        process::Command,
        time::Duration,
    };

    use shuttle_common::database;
    use shuttle_service::Factory;
    use tokio::{
        sync::{broadcast, oneshot},
        task::JoinError,
        time::sleep,
    };
    use uuid::Uuid;

    use crate::{deployment::queue::LIBS_PATH, error::Error};

    use super::Built;

    const RESOURCES_PATH: &str = "tests/resources";

    struct StubFactory;

    #[async_trait::async_trait]
    impl Factory for StubFactory {
        async fn get_db_connection_string(
            &mut self,
            _db_type: database::Type,
        ) -> Result<String, shuttle_service::Error> {
            panic!("no run test should get an sql connection");
        }

        async fn get_secrets(
            &mut self,
        ) -> Result<BTreeMap<String, String>, shuttle_service::Error> {
            panic!("no test should get any secrets");
        }
    }

    struct StubLogger;
    impl log::Log for StubLogger {
        fn enabled(&self, _metadata: &log::Metadata) -> bool {
            false
        }

        fn log(&self, _record: &log::Record) {}

        fn flush(&self) {}
    }

    // This test uses the kill signal to make sure a service does stop when asked to
    #[tokio::test]
    async fn can_be_killed() {
        let built = make_so_and_built("sleep-async");
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
        let addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 8001);
        let mut factory = StubFactory;
        let logger = Box::new(StubLogger);

        built
            .handle(addr, &mut factory, logger, kill_recv, handle_cleanup)
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
        let built = make_so_and_built("sleep-async");
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
        let addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 8001);
        let mut factory = StubFactory;
        let logger = Box::new(StubLogger);

        built
            .handle(addr, &mut factory, logger, kill_recv, handle_cleanup)
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
        let built = make_so_and_built("bind-panic");
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
        let addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 8001);
        let mut factory = StubFactory;
        let logger = Box::new(StubLogger);

        built
            .handle(addr, &mut factory, logger, kill_recv, handle_cleanup)
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
        let built = make_so_and_built("main-panic");
        let (_kill_send, kill_recv) = broadcast::channel(1);

        let handle_cleanup = |_result| panic!("the service shouldn't even start");
        let addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 8001);
        let mut factory = StubFactory;
        let logger = Box::new(StubLogger);

        let result = built
            .handle(addr, &mut factory, logger, kill_recv, handle_cleanup)
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
            name: "test".to_string(),
        };
        let (_kill_send, kill_recv) = broadcast::channel(1);

        let handle_cleanup = |_result| panic!("no service means no cleanup");
        let addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 8001);
        let mut factory = StubFactory;
        let logger = Box::new(StubLogger);

        let result = built
            .handle(addr, &mut factory, logger, kill_recv, handle_cleanup)
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

    fn make_so_and_built(crate_name: &str) -> Built {
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
        let new_so_path = PathBuf::from(LIBS_PATH).join(id.to_string());

        std::fs::copy(so_path, new_so_path).unwrap();

        Built {
            id,
            name: crate_name.to_string(),
        }
    }
}
