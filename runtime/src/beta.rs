use std::{
    collections::BTreeMap,
    iter::FromIterator,
    net::{Ipv4Addr, SocketAddr},
    process::exit,
};

use anyhow::Context;
use shuttle_api_client::ShuttleApiClient;
use shuttle_common::{resource::ResourceInput, secrets::Secret};
use shuttle_service::{ResourceFactory, Service};

use crate::__internals::{Loader, Runner};

pub async fn start(loader: impl Loader + Send + 'static, runner: impl Runner + Send + 'static) {
    // TODO: parse all of the below from env vars
    let secrets = BTreeMap::new();
    let project_name = "TODO".to_owned();
    let env = "production".parse().unwrap();
    let ip = Ipv4Addr::UNSPECIFIED;
    let port = 3000;
    let api_url = "http://0.0.0.0:8000".to_owned(); // runner proxy

    let addr = SocketAddr::new(ip.into(), port);

    // Sorts secrets by key
    let secrets = BTreeMap::from_iter(secrets.into_iter().map(|(k, v)| (k, Secret::new(v))));

    let factory = ResourceFactory::new(project_name, secrets, env);

    let mut resources = match loader.load(factory).await {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Runtime Loader phase failed: {e}");
            exit(101);
        }
    };

    let client = ShuttleApiClient::new(api_url, None, None);

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
            exit(101);
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
            .provision_resource_beta(
                "self", /* TODO use actual name for c-s knowledge */
                shuttle_resource,
            )
            .await
        {
            Ok(o) => o,
            Err(e) => {
                eprintln!("Runtime Provisioning phase failed: {e}");
                exit(103);
            }
        };
        *bytes = serde_json::to_vec(&o).expect("to serialize struct");
    }

    // TODO?: call API to say running state is being entered

    let service = match runner.run(resources).await {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Runtime Runner phase failed: {e}");
            exit(105);
        }
    };

    if let Err(e) = service.bind(addr).await {
        eprintln!("Service encountered an error: {e}");
        exit(1);
    }
}
