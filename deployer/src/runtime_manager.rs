use std::{collections::HashMap, net::Ipv4Addr, sync::Arc, time::Duration};

use anyhow::Context;
use shuttle_common::claims::{ClaimLayer, ClaimService, InjectPropagation, InjectPropagationLayer};
use shuttle_proto::runtime::{
    runtime_client::{self, RuntimeClient},
    Ping, StopRequest, SubscribeLogsRequest,
};
use tonic::transport::{Channel, Endpoint};
use tower::ServiceBuilder;
use tracing::{debug, info, trace};
use ulid::Ulid;

use crate::project::service::RUNTIME_API_PORT;

type Runtimes =
    Arc<tokio::sync::Mutex<HashMap<Ulid, RuntimeClient<ClaimService<InjectPropagation<Channel>>>>>>;

/// Manager that can start up mutliple runtimes. This is needed so that two runtimes can be up when a new deployment is made:
/// One runtime for the new deployment being loaded; another for the currently active deployment
#[derive(Clone, Default)]
pub struct RuntimeManager {
    runtimes: Runtimes,
}

impl RuntimeManager {
    pub async fn runtime_client(
        &mut self,
        service_id: Ulid,
        target_ip: Ipv4Addr,
    ) -> anyhow::Result<RuntimeClient<ClaimService<InjectPropagation<Channel>>>> {
        let mut guard = self.runtimes.lock().await;

        if let Some(runtime_client) = guard.get(&service_id) {
            return Ok(runtime_client.clone());
        }

        // Connection to the docker container where the shuttle-runtime lives.
        let conn = Endpoint::new(format!("http://{target_ip}:{RUNTIME_API_PORT}"))
            .context("creating runtime client endpoint")?
            .connect_timeout(Duration::from_secs(5));

        let channel = conn.connect().await.context("connecting runtime client")?;
        let channel = ServiceBuilder::new()
            .layer(ClaimLayer)
            .layer(InjectPropagationLayer)
            .service(channel);
        let runtime_client = runtime_client::RuntimeClient::new(channel);
        guard.insert(service_id, runtime_client.clone());

        Ok(runtime_client)
    }

    /// Send a kill / stop signal for a deployment to its running runtime
    pub async fn kill(&mut self, id: &Ulid) -> bool {
        let value = self.runtimes.lock().await.remove(id);

        if let Some(mut runtime_client) = value {
            trace!(%id, "sending stop signal for deployment");

            let stop_request = tonic::Request::new(StopRequest {});
            let response = runtime_client.stop(stop_request).await.unwrap();
            trace!(?response, "stop deployment response");

            response.into_inner().success
        } else {
            trace!("no client running");
            true
        }
    }

    pub async fn is_healthy(&self, service_id: &Ulid) -> bool {
        let mut guard = self.runtimes.lock().await;

        if let Some(runtime_client) = guard.get_mut(service_id) {
            trace!(%service_id, "sending ping to the runtime");

            let ping = tonic::Request::new(Ping {});
            let response = runtime_client.health_check(ping).await;
            match response {
                Ok(_) => {
                    trace!("runtime responded with pong");
                    true
                }
                Err(status) => {
                    trace!(?status, "health check failed");
                    false
                }
            }
        } else {
            info!("no client running");
            false
        }
    }

    pub async fn logs_subscribe(&self, service_id: &Ulid) -> anyhow::Result<()> {
        let mut stream = self
            .runtimes
            .lock()
            .await
            .get_mut(service_id)
            .context(format!("No runtime client for deployment {service_id}"))?
            .subscribe_logs(tonic::Request::new(SubscribeLogsRequest {}))
            .await
            .context("subscribing to runtime logs stream")?
            .into_inner();

        tokio::spawn(async move {
            while let Ok(Some(log)) = stream.message().await {
                println!("{log:?}");
            }
        });

        Ok(())
    }
}
