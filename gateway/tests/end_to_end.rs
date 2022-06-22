use std::io::Read;
use std::sync::Arc;
use std::time::Duration;

use axum::headers::{
    Authorization
};
use futures::prelude::*;
use http::Request;

use hyper::{
    Body,
    StatusCode
};
use log::info;
use shuttle_common::{
    DeploymentMeta,
    DeploymentStateMeta
};
use shuttle_gateway::api::make_api;
use shuttle_gateway::auth::User;
use shuttle_gateway::project::Project;
use shuttle_gateway::proxy::make_proxy;
use shuttle_gateway::service::GatewayService;
use shuttle_gateway::tests::{
    RequestBuilderExt,
    World
};
use shuttle_gateway::worker::Worker;
use tokio::sync::mpsc::channel;

macro_rules! timed_loop {
    (wait: $wait:literal$(, max: $max:literal)?, $block:block) => {{
        #[allow(unused_mut)]
        #[allow(unused_variables)]
        let mut tries = 0;
        loop {
            $block
            tries += 1;
            $(if tries > $max {
                panic!("timed out in the loop");
            })?
            ::tokio::time::sleep(::std::time::Duration::from_secs($wait)).await;
        }
    }};
}

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
    let api_addr = format!("127.0.0.1:{}", base_port).parse().unwrap();
    let serve_api = hyper::Server::bind(&api_addr).serve(api.into_make_service());
    let api_client = world.client(api_addr.clone());

    let proxy = make_proxy(Arc::clone(&service));
    let proxy_addr = format!("127.0.0.1:{}", base_port + 1).parse().unwrap();
    let serve_proxy = hyper::Server::bind(&proxy_addr).serve(proxy);
    let proxy_client = world.client(proxy_addr.clone());

    let _gateway = tokio::spawn(async move {
        tokio::select! {
            _ = worker.start() => {},
            _ = serve_api => {},
            _ = serve_proxy => {}
        }
    });

    let User { key, name, .. } = service.create_user("neo".parse().unwrap()).await.unwrap();
    service.set_super_user(&name, true).await.unwrap();

    let User { key, .. } = api_client
        .request(
            Request::post("/users/trinity")
                .with_header(&Authorization::basic("", key.as_str()))
                .body(Body::empty())
                .unwrap()
        )
        .map_ok(|resp| {
            assert_eq!(resp.status(), StatusCode::OK);
            serde_json::from_slice(resp.body()).unwrap()
        })
        .await
        .unwrap();

    let authorization = Authorization::basic("", key.as_str());

    api_client
        .request(
            Request::post("/projects/matrix")
                .with_header(&authorization)
                .body(Body::empty())
                .unwrap()
        )
        .map_ok(|resp| {
            assert_eq!(resp.status(), StatusCode::OK);
        })
        .await
        .unwrap();

    let _ = timed_loop!(wait: 1, max: 12, {
        let project: Project = api_client
            .request(
                Request::get("/projects/matrix")
                    .with_header(&authorization)
                    .body(Body::empty())
                    .unwrap(),
            )
            .map_ok(|resp| {
                assert_eq!(resp.status(), StatusCode::OK);
                serde_json::from_slice(resp.body()).unwrap()
            })
            .await
            .unwrap();

        // Equivalent to `::Ready(_)`
        if let Some(target_ip) = project.target_addr().unwrap() {
            break target_ip;
        }

        tokio::time::sleep(Duration::from_secs(1)).await;
    });

    api_client
        .request(
            Request::get("/projects/matrix/status")
                .with_header(&authorization)
                .body(Body::empty())
                .unwrap()
        )
        .map_ok(|resp| assert_eq!(resp.status(), StatusCode::OK))
        .await
        .unwrap();

    // === deployment test BEGIN ===
    api_client
        .request({
            let mut data = Vec::new();
            let mut f = std::fs::File::open("tests/hello_world.crate").unwrap();
            f.read_to_end(&mut data).unwrap();
            Request::post("/projects/matrix/projects/matrix")
                .with_header(&authorization)
                .body(Body::from(data))
                .unwrap()
        })
        .map_ok(|resp| assert_eq!(resp.status(), StatusCode::OK))
        .await
        .unwrap();

    timed_loop!(wait: 1, max: 600, {
        let meta: DeploymentMeta = api_client
            .request(
                Request::get("/projects/matrix/projects/matrix")
                    .with_header(&authorization)
                    .body(Body::empty())
                    .unwrap(),
            )
            .map_ok(|resp| {
                assert_eq!(resp.status(), StatusCode::OK);
                serde_json::from_slice(resp.body()).unwrap()
            })
            .await
            .unwrap();
        if matches!(meta.state, DeploymentStateMeta::Deployed) {
            break;
        }
    });

    proxy_client
        .request(
            Request::get("/hello")
                .header("Host", "matrix.shuttleapp.rs")
                .body(Body::empty())
                .unwrap()
        )
        .map_ok(|resp| {
            assert_eq!(resp.status(), StatusCode::OK);
            assert_eq!(
                String::from_utf8(resp.into_body()).unwrap().as_str(),
                "Hello, world!"
            );
        });
    // === deployment test END ===

    api_client
        .request(
            Request::delete("/projects/matrix")
                .with_header(&authorization)
                .body(Body::empty())
                .unwrap()
        )
        .map_ok(|resp| assert_eq!(resp.status(), StatusCode::OK))
        .await
        .unwrap();

    timed_loop!(wait: 1, max: 12, {
        let project: Project = api_client
            .request(
                Request::get("/projects/matrix")
                    .with_header(&authorization)
                    .body(Body::empty())
                    .unwrap(),
            )
            .map_ok(|resp| {
                assert_eq!(resp.status(), StatusCode::OK);
                serde_json::from_slice(resp.body()).unwrap()
            })
            .await
            .unwrap();
        if matches!(project, Project::Destroyed(_)) {
            break;
        }
    });
}
