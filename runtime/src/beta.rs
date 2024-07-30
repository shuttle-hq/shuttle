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
use shuttle_common::{
    resource::{ResourceInput, ResourceState, Type},
    secrets::Secret,
};
use shuttle_service::{ResourceFactory, Service};

use crate::__internals::{Loader, Runner};

const HEALTH_CHECK_PORT: u16 = 8001;

pub async fn start(loader: impl Loader + Send + 'static, runner: impl Runner + Send + 'static) {
    // Uses primitive parsing instead of clap for reduced dependency weight
    let shuttle = std::env::var("SHUTTLE").is_ok();
    let project_id = std::env::var("SHUTTLE_PROJECT_ID").expect("project id env var");
    let project_name = std::env::var("SHUTTLE_PROJECT_NAME").expect("project name env var");
    let env = std::env::var("SHUTTLE_ENV")
        .expect("shuttle environment env var")
        .parse()
        .expect("invalid shuttle environment");
    let ip = std::env::var("SHUTTLE_RUNTIME_IP")
        .expect("runtime ip env var")
        .parse()
        .expect("invalid ip");
    let port = std::env::var("SHUTTLE_RUNTIME_PORT")
        .expect("runtime port env var")
        .parse()
        .expect("invalid port");
    let api_url = std::env::var("SHUTTLE_API").expect("api env var");

    let service_addr = SocketAddr::new(ip, port);
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
    if shuttle {
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
                exit(200);
            }
        });
    }

    // Sorts secrets by key
    let secrets = BTreeMap::from_iter(secrets.into_iter().map(|(k, v)| (k, Secret::new(v))));

    let factory = ResourceFactory::new(project_name, secrets.clone(), env);

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
        // Secrets don't need to be requested here since we already got them above.
        if shuttle_resource.r#type == Type::Secrets {
            *bytes = serde_json::to_vec(&secrets).expect("to serialize struct");
            continue;
        }
        // TODO?: Add prints/tracing to show which resource is being provisioned
        loop {
            match client
                .provision_resource_beta(&project_id, shuttle_resource.clone())
                .await
            {
                Ok(o) => match o.state.expect("resource to have a state") {
                    ResourceState::Provisioning | ResourceState::Authorizing => {
                        tokio::time::sleep(tokio::time::Duration::from_millis(2000)).await;
                    }
                    ResourceState::Ready => {
                        *bytes = serde_json::to_vec(&o.output).expect("to serialize struct");
                        break;
                    }
                    bad_state => {
                        eprintln!(
                            "Runtime Provisioning phase failed: Received '{}' resource with state '{}'.",
                            shuttle_resource.r#type,
                            bad_state
                        );
                        exit(132);
                    }
                },
                Err(e) => {
                    eprintln!("Runtime Provisioning phase failed: {e}");
                    exit(131);
                }
            };
        }
    }

    // TODO?: call API to say running state is being entered

    // Tell sidecar to shut down.
    // Ignore error, since the endpoint does not send a response.
    let _ = client.client.get("/__shuttle/shutdown").send().await;

    let service = match runner.run(resources).await {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Runtime Resource Initialization phase failed: {e}");
            exit(151);
        }
    };

    if let Err(e) = service.bind(service_addr).await {
        eprintln!("Service encountered an error in `bind`: {e}");
        exit(1);
    }
}
