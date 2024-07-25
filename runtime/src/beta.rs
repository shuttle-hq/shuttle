use std::{
    collections::BTreeMap,
    iter::FromIterator,
    net::{Ipv4Addr, SocketAddr},
    process::exit,
};

use shuttle_common::secrets::Secret;
use shuttle_service::{ResourceFactory, Service};

use crate::__internals::{Loader, Runner};

pub async fn start(loader: impl Loader + Send + 'static, runner: impl Runner + Send + 'static) {
    // TODO: parse all of the below from env vars
    let secrets = BTreeMap::new();
    let project_name = "TODO".to_owned();
    let env = "production".parse().unwrap();
    let ip = Ipv4Addr::UNSPECIFIED;
    let port = 3000;

    let addr = SocketAddr::new(ip.into(), port);

    // Sorts secrets by key
    let secrets = BTreeMap::from_iter(secrets.into_iter().map(|(k, v)| (k, Secret::new(v))));

    let factory = ResourceFactory::new(project_name, secrets, env);

    let resources = match loader.load(factory).await {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Runtime Loader phase failed: {e}");
            exit(101);
        }
    };

    // TODO: loop and request each resource

    // TODO?: call API to say running state is being entered

    let service = match runner.run(resources).await {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Runtime Runner phase failed: {e}");
            exit(105);
        }
    };

    if let Err(e) = service.bind(addr).await {
        eprintln!("Runtime service encountered an error: {e}");
        exit(1);
    }
}
