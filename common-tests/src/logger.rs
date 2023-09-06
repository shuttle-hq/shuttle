use std::{
    net::{Ipv4Addr, SocketAddr},
    time::Duration,
};

use portpicker::pick_unused_port;
use shuttle_common::claims::{ClaimLayer, InjectPropagationLayer};
use shuttle_proto::logger::{
    logger_client::LoggerClient,
    logger_server::{Logger, LoggerServer},
};
use tonic::transport::{Endpoint, Server};
use tower::ServiceBuilder;

pub async fn mocked_logger_client(
    logger: impl Logger,
) -> LoggerClient<
    shuttle_common::claims::ClaimService<
        shuttle_common::claims::InjectPropagation<tonic::transport::Channel>,
    >,
> {
    let logger_addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), pick_unused_port().unwrap());
    let logger_uri = format!("http://{}", logger_addr);
    tokio::spawn(async move {
        Server::builder()
            .add_service(LoggerServer::new(logger))
            .serve(logger_addr)
            .await
    });

    // Wait for the logger server to start before creating a client.
    tokio::time::sleep(Duration::from_millis(200)).await;

    let channel = Endpoint::try_from(logger_uri.to_string())
        .unwrap()
        .connect()
        .await
        .expect("failed to connect to logger");

    let channel = ServiceBuilder::new()
        .layer(ClaimLayer)
        .layer(InjectPropagationLayer)
        .service(channel);

    LoggerClient::new(channel)
}
