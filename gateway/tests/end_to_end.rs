use std::sync::Arc;

use tokio::sync::mpsc::channel;

use log::info;

use shuttle_gateway::{
    api::make_api, proxy::make_proxy, service::GatewayService, tests::World, worker::Worker,
};

#[tokio::test]
async fn end_to_end() {
    let world = World::new().await.unwrap();
    let service = Arc::new(GatewayService::init(world.context().args.clone()).await);

    let worker = Worker::new(Arc::clone(&service));

    let (log_out, mut log_in) = channel(256);

    tokio::spawn({
        let sender = worker.sender();
        async move {
            while let Some(work) = log_in.recv().await {
                info!("work: {work:?}");
                sender.send(work).await.unwrap()
            }
            info!("work channel closed");
        }
    });

    service.set_sender(Some(log_out)).await.unwrap();

    let base_port = loop {
        let port = portpicker::pick_unused_port().unwrap();
        if portpicker::is_free_tcp(port + 1) {
            break port;
        }
    };

    let api = make_api(Arc::clone(&service));
    let serve_api = hyper::Server::bind(&format!("127.0.0.1:{}", base_port).parse().unwrap())
        .serve(api.into_make_service());

    let proxy = make_proxy(Arc::clone(&service));
    let serve_proxy =
        hyper::Server::bind(&format!("127.0.0.1:{}", base_port + 1).parse().unwrap()).serve(proxy);

    let gateway = tokio::spawn(async move {
        tokio::select! {
        _ = serve_api => {},
        _ = serve_proxy => {}
        }
    });

    
}
