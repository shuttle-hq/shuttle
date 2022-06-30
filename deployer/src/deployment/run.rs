use std::{
    net::{Ipv4Addr, SocketAddr},
    path::PathBuf,
    str::FromStr,
};

use portpicker::pick_unused_port;
use shuttle_common::project::ProjectName;
use shuttle_service::{loader::Loader, Factory};
use tracing::{debug, error, info, instrument};

use super::{provisioner_factory::AbstractFactory, KillReceiver, KillSender, RunReceiver, State};
use crate::error::{Error, Result};

/// Run a task which takes runnable deploys from a channel and starts them up with factory provided by the abstract factory
/// A deploy is killed when it receives a signal from the kill channel
pub async fn task(
    mut recv: RunReceiver,
    kill_send: KillSender,
    abstract_factory: impl AbstractFactory,
) {
    info!("Run task started");

    while let Some(built) = recv.recv().await {
        let name = built.name.clone();

        info!("Built deployment at the front of run queue: {}", name);

        let kill_recv = kill_send.subscribe();

        let abstract_factory = abstract_factory.clone();

        tokio::spawn(async move {
            let port = pick_unused_port().unwrap();
            let addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), port);
            let mut factory = abstract_factory.get_factory(ProjectName::from_str(&name).unwrap());

            if let Err(e) = built
                .handle(addr, &mut factory, kill_recv, |_built| {})
                .await
            {
                error!("Error during running of deployment '{}' - {e}", name);
            }
        });
    }
}

#[derive(Debug)]
pub struct Built {
    pub name: String,
    pub so_path: PathBuf,
}

struct StubLogger;

impl log::Log for StubLogger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        todo!()
    }

    fn log(&self, record: &log::Record) {
        todo!()
    }

    fn flush(&self) {
        todo!()
    }
}

impl Built {
    #[instrument(skip(self, factory, handle_cleanup), fields(name = self.name.as_str(), state = %State::Running))]
    async fn handle(
        self,
        addr: SocketAddr,
        factory: &mut dyn Factory,
        mut kill_recv: KillReceiver,
        handle_cleanup: impl FnOnce(Result<()>) + Send + 'static,
    ) -> Result<()> {
        let loader = Loader::from_so_file(self.so_path.clone())?;

        let logger = Box::new(StubLogger);
        let (mut handle, library) = loader.load(factory, addr, logger).await.unwrap();

        // Execute loaded service:

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

            let result = match result {
                Ok(result) => result.map_err(|e| Error::Run(e.into())),
                Err(error) if error.is_cancelled() => Ok(()),
                _ => todo!(),
            };

            library.close().unwrap();

            handle_cleanup(result);
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
            todo!()
        }
    }

    #[tokio::test]
    async fn can_be_killed() {
        let built = make_so_create_built("sleep-async");
        let (kill_send, kill_recv) = broadcast::channel(1);
        let (cleanup_send, cleanup_recv) = oneshot::channel();

        let handle_cleanup = |_built| {
            cleanup_send.send(()).unwrap();
        };
        let addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 8001);
        let mut factory = StubFactory;

        built
            .handle(addr, &mut factory, kill_recv, handle_cleanup)
            .await
            .unwrap();

        // Give it some time to start up
        sleep(Duration::from_secs(1)).await;

        kill_send.send("sleep-async".to_string()).unwrap();

        tokio::select! {
            _ = sleep(Duration::from_secs(1)) => panic!("cleanup should have been called"),
            _ = cleanup_recv => {}
        }
    }

    #[tokio::test]
    async fn self_stop() {
        let built = make_so_create_built("sleep-async");
        let (_kill_send, kill_recv) = broadcast::channel(1);
        let (cleanup_send, cleanup_recv) = oneshot::channel();

        let handle_cleanup = |_built| {
            cleanup_send.send(()).unwrap();
        };
        let addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 8001);
        let mut factory = StubFactory;

        built
            .handle(addr, &mut factory, kill_recv, handle_cleanup)
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

        let result = built
            .handle(addr, &mut factory, kill_recv, handle_cleanup)
            .await;

        assert!(matches!(result, Err(Error::Load(_))));
    }

    fn make_so_create_built(crate_name: &str) -> Built {
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
