// TODO: client should create project then push new deployment (refactor endpoint)
// TODO: Add some tests (ideas?)
// TODO: Implement the delete project endpoint to make sure users can
//       self-serve out of issues

use std::io;
use std::sync::Arc;

use clap::Parser;
use futures::prelude::*;
use log::{error, info};

use shuttle_gateway::{
    api::make_api, args::Args, proxy::make_proxy, service::GatewayService, worker::Worker,
};

#[tokio::main]
async fn main() -> io::Result<()> {
    env_logger::init();

    let args = Args::parse();

    let gateway = Arc::new(GatewayService::init(args.clone()).await);

    let worker = Worker::new(Arc::clone(&gateway));
    gateway.set_sender(Some(worker.sender())).await.unwrap();
    gateway
        .refresh()
        .map_err(|err| error!("failed to refresh state at startup: {err}"))
        .await
        .unwrap();

    let worker_handle = tokio::spawn(
        worker
            .start()
            .map_ok(|_| info!("worker terminated successfully"))
            .map_err(|err| error!("worker error: {}", err)),
    );

    let api = make_api(Arc::clone(&gateway));

    let api_handle = tokio::spawn(axum::Server::bind(&args.control).serve(api.into_make_service()));

    let proxy = make_proxy(gateway);

    let proxy_handle = tokio::spawn(hyper::Server::bind(&args.user).serve(proxy));

    let _ = tokio::join!(worker_handle, api_handle, proxy_handle);

    Ok(())
}
