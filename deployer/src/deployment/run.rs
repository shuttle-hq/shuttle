use std::{
    net::{Ipv4Addr, SocketAddr},
    path::PathBuf,
    str::FromStr,
};

use portpicker::pick_unused_port;
use shuttle_common::project::ProjectName;
use shuttle_service::{loader::Loader, Factory};
use tokio::task::JoinError;
use tracing::{debug, error, info, instrument};

use super::{provisioner_factory, runtime_logger, KillReceiver, KillSender, RunReceiver, State};
use crate::error::Result;

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
        let name = built.name.clone();

        info!("Built deployment at the front of run queue: {}", name);

        let kill_recv = kill_send.subscribe();

        let port = pick_unused_port().unwrap();
        let addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), port);
        let mut factory = abstract_factory.get_factory(ProjectName::from_str(&name).unwrap());
        let logger = logger_factory.get_logger(name.clone());
        let cleanup_name = name.clone();
        let cleanup = move |result: std::result::Result<anyhow::Result<()>, JoinError>| match result
        {
            Ok(inner) => match inner {
                Ok(()) => completed_cleanup(&cleanup_name),
                Err(err) => crashed_cleanup(&cleanup_name, err),
            },
            Err(err) if err.is_cancelled() => stopped_cleanup(&cleanup_name),
            Err(err) => start_crashed_cleanup(&cleanup_name, err),
        };

        tokio::spawn(async move {
            if let Err(err) = built
                .handle(addr, &mut factory, logger, kill_recv, cleanup)
                .await
            {
                start_crashed_cleanup(&name, err)
            }
        });
    }
}

#[instrument(fields(name = _name, state = %State::Completed))]
fn completed_cleanup(_name: &str) {
    info!("service finished all on its own");
}

#[instrument(fields(name = _name, state = %State::Stopped))]
fn stopped_cleanup(_name: &str) {
    info!("service was stopped by the user");
}

#[instrument(fields(name = _name, state = %State::Crashed))]
fn crashed_cleanup(_name: &str, err: anyhow::Error) {
    let error: &dyn std::error::Error = err.as_ref();
    error!(error, "service encountered an error");
}

#[instrument(fields(name = _name, state = %State::Crashed))]
fn start_crashed_cleanup(_name: &str, err: impl std::error::Error + 'static) {
    error!(
        error = &err as &dyn std::error::Error,
        "service startup encountered an error"
    );
}

#[derive(Debug)]
pub struct Built {
    pub name: String,
    pub so_path: PathBuf,
}

impl Built {
    #[instrument(skip(self, factory, logger, cleanup), fields(name = self.name.as_str(), state = %State::Running))]
    async fn handle(
        self,
        addr: SocketAddr,
        factory: &mut dyn Factory,
        logger: Box<dyn log::Log>,
        mut kill_recv: KillReceiver,
        cleanup: impl FnOnce(std::result::Result<anyhow::Result<()>, JoinError>) + Send + 'static,
    ) -> Result<()> {
        let loader = Loader::from_so_file(self.so_path.clone())?;

        let (mut handle, library) = loader.load(factory, addr, logger).await?;

        // Execute loaded service
        tokio::spawn(async move {
            let result;
            loop {
                tokio::select! {
                     Ok(name) = kill_recv.recv() => {
                         if name == self.name {
                             debug!("Service {name} killed");
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

            library.close().unwrap();

            cleanup(result);
        });

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::{
        net::{Ipv4Addr, SocketAddr},
        path::PathBuf,
        process::Command,
        time::Duration,
    };

    use shuttle_service::Factory;
    use tokio::{
        sync::{broadcast, oneshot},
        task::JoinError,
        time::sleep,
    };

    use crate::error::Error;

    use super::Built;

    const RESOURCES_PATH: &str = "tests/resources";

    struct StubFactory;

    #[async_trait::async_trait]
    impl Factory for StubFactory {
        async fn get_sql_connection_string(
            &mut self,
        ) -> core::result::Result<String, shuttle_service::Error> {
            panic!("no run test should get an sql connection");
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
        let built = make_so_create_and_built("sleep-async");
        let (kill_send, kill_recv) = broadcast::channel(1);
        let (cleanup_send, cleanup_recv) = oneshot::channel();

        let handle_cleanup = |result: std::result::Result<anyhow::Result<()>, JoinError>| {
            assert!(
                result.unwrap_err().is_cancelled(),
                "handle should have been cancelled"
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
        kill_send.send("sleep-async".to_string()).unwrap();

        tokio::select! {
            _ = sleep(Duration::from_secs(1)) => panic!("cleanup should have been called"),
            _ = cleanup_recv => {}
        }
    }

    // This test does not use a kill signal to stop the service. Rather the service decided to stop on its own without errors
    #[tokio::test]
    async fn self_stop() {
        let built = make_so_create_and_built("sleep-async");
        let (_kill_send, kill_recv) = broadcast::channel(1);
        let (cleanup_send, cleanup_recv) = oneshot::channel();

        let handle_cleanup = |result: std::result::Result<anyhow::Result<()>, JoinError>| {
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
            _ = cleanup_recv => {}
        }
    }

    // Test for panics in Service::bind
    #[tokio::test]
    async fn panic_in_bind() {
        let built = make_so_create_and_built("bind-panic");
        let (_kill_send, kill_recv) = broadcast::channel(1);

        let handle_cleanup = |_result| panic!("handle from service should never start");
        let addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 8001);
        let mut factory = StubFactory;
        let logger = Box::new(StubLogger);

        let result = built
            .handle(addr, &mut factory, logger, kill_recv, handle_cleanup)
            .await;

        assert_eq!(
            result.unwrap_err().to_string(),
            "Run error: Panic occurred in `Service::bind`: panic in bind"
        );
    }

    // Test for panics in handle returned from Service::bind
    #[tokio::test]
    async fn panic_in_bind_handle() {
        let built = make_so_create_and_built("handle-panic");
        let (_kill_send, kill_recv) = broadcast::channel(1);
        let (cleanup_send, cleanup_recv) = oneshot::channel();

        let handle_cleanup = |result: std::result::Result<anyhow::Result<()>, JoinError>| {
            let result = result.unwrap();
            assert!(result.is_err(), "expected inner error from handle");
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
            _ = cleanup_recv => {}
        }
    }

    #[tokio::test]
    async fn missing_so() {
        let built = Built {
            name: "test".to_string(),
            so_path: PathBuf::from("missing.so"),
        };
        let (_kill_send, kill_recv) = broadcast::channel(1);

        let handle_cleanup = |_built| {};
        let addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 8001);
        let mut factory = StubFactory;
        let logger = Box::new(StubLogger);

        let result = built
            .handle(addr, &mut factory, logger, kill_recv, handle_cleanup)
            .await;

        assert!(matches!(result, Err(Error::Load(_))));
    }

    fn make_so_create_and_built(crate_name: &str) -> Built {
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

        let so_path = crate_dir.join("target/release").join(lib_name);

        Built {
            name: crate_name.to_string(),
            so_path,
        }
    }
}
