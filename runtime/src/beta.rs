use std::{
    collections::BTreeMap,
    convert::Infallible,
    iter::FromIterator,
    net::{Ipv4Addr, SocketAddr},
    process::exit,
};

use anyhow::Context;
use hyper::{
    service::{make_service_fn, service_fn},
    Body, Response, Server,
};
use shuttle_api_client::ShuttleApiClient;
use shuttle_common::{resource::ResourceInput, secrets::Secret};
use shuttle_service::{ResourceFactory, Service};

use crate::__internals::{Loader, Runner};

const HEALTH_CHECK_PORT: u16 = 8001;

pub async fn start(loader: impl Loader + Send + 'static, runner: impl Runner + Send + 'static) {
    // TODO: parse all of the below from env vars
    let project_id = "proj_TODO".to_owned();
    let project_name = "TODO".to_owned();
    let env = "production".parse().unwrap();
    let ip = Ipv4Addr::UNSPECIFIED;
    let port = 3000;
    let api_url = "http://0.0.0.0:8000".to_owned(); // runner proxy

    let service_addr = SocketAddr::new(ip.into(), port);
    let client = ShuttleApiClient::new(api_url, None, None);

    let secrets: BTreeMap<String, String> = match client
        .get_secrets_beta(&project_id)
        .await
        .and_then(|r| serde_json::from_value(r.data).context("failed to deserialize secrets"))
    {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Runtime Secret Loading phase failed: {e}");
            exit(101);
        }
    };

    // if running on Shuttle, start a health check server
    if std::env::var("SHUTTLE").is_ok() {
        tokio::task::spawn(async move {
            let make_service = make_service_fn(|_conn| async {
                Ok::<_, Infallible>(service_fn(|_req| async move {
                    Result::<Response<Body>, hyper::Error>::Ok(Response::new(Body::empty()))
                }))
            });
            let server = Server::bind(&SocketAddr::new(
                Ipv4Addr::LOCALHOST.into(),
                HEALTH_CHECK_PORT,
            ))
            .serve(make_service);

            if let Err(e) = server.await {
                eprintln!("Internal health check error: {}", e);
                exit(150);
            }
        });
    }

    // Sorts secrets by key
    let secrets = BTreeMap::from_iter(secrets.into_iter().map(|(k, v)| (k, Secret::new(v))));

    let factory = ResourceFactory::new(project_name, secrets, env);

    let mut resources = match loader.load(factory).await {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Runtime Loader phase failed: {e}");
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
            eprintln!("Runtime Provisioning phase failed: {e}");
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
        let o = match client
            .provision_resource_beta(&project_id, shuttle_resource)
            .await
        {
            Ok(o) => o,
            Err(e) => {
                eprintln!("Runtime Provisioning phase failed: {e}");
                exit(131);
            }
        };
        *bytes = serde_json::to_vec(&o).expect("to serialize struct");
    }

    // TODO?: call API to say running state is being entered

    // call sidecar to shut down. ignore error, since the endpoint does not send a response
    let _ = client.client.get("/__shuttle/shutdown").send().await;

    let service = match runner.run(resources).await {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Runtime Runner phase failed: {e}");
            exit(151);
        }
    };

    if let Err(e) = service.bind(service_addr).await {
        eprintln!("Service encountered an error: {e}");
        exit(1);
    }
}
