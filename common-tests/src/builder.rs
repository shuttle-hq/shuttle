use std::{
    net::{Ipv4Addr, SocketAddr},
    time::Duration,
};

use portpicker::pick_unused_port;
use shuttle_common::claims::{ClaimLayer, InjectPropagationLayer};
use shuttle_proto::builder::{
    builder_client::BuilderClient,
    builder_server::{Builder, BuilderServer},
};
use tonic::transport::{Endpoint, Server};
use tower::ServiceBuilder;

pub async fn get_mocked_builder_client(
    builder: impl Builder,
) -> BuilderClient<
    shuttle_common::claims::ClaimService<
        shuttle_common::claims::InjectPropagation<tonic::transport::Channel>,
    >,
> {
    let builder_addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), pick_unused_port().unwrap());
    let builder_uri = format!("http://{}", builder_addr);
    tokio::spawn(async move {
        Server::builder()
            .add_service(BuilderServer::new(builder))
            .serve(builder_addr)
            .await
    });

    // Wait for the builder server to start before creating a client.
    tokio::time::sleep(Duration::from_millis(200)).await;

    let channel = Endpoint::try_from(builder_uri.to_string())
        .unwrap()
        .connect()
        .await
        .expect("failed to connect to builder");

    let channel = ServiceBuilder::new()
        .layer(ClaimLayer)
        .layer(InjectPropagationLayer)
        .service(channel);

    BuilderClient::new(channel)
}
