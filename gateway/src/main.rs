use clap::Parser;

use shuttle_common::backends::tracing::setup_tracing;
use shuttle_gateway::acme::{AcmeClient, CustomDomain};
use shuttle_gateway::api::latest::ApiBuilder;
use shuttle_gateway::args::{Args, UseTls};
use shuttle_gateway::dal::Sqlite;
use shuttle_gateway::proxy::UserServiceBuilder;
use shuttle_gateway::service::GatewayService;
use shuttle_gateway::tls::make_tls_acceptor;
use std::io::{self, Cursor};

use std::sync::Arc;
use tracing::{debug, error, trace, warn};

#[tokio::main(flavor = "multi_thread")]
async fn main() -> io::Result<()> {
    let args = Args::parse();

    trace!(args = ?args, "parsed args");

    setup_tracing(tracing_subscriber::registry(), "gateway");

    let db_path = args.state.join("gateway.sqlite");

    let sqlite = Sqlite::new(db_path.to_str().unwrap()).await;

    start(sqlite, args).await
}

async fn start(db: Sqlite, args: Args) -> io::Result<()> {
    let gateway_service =
        Arc::new(GatewayService::init(db, args.state, args.proxy_fqdn.clone()).await);

    let acme_client = AcmeClient::new();

    let mut api_builder = ApiBuilder::new()
        .with_service(gateway_service.clone())
        .binding_to(args.control);

    let proxy_fqdn = args.proxy_fqdn.clone();

    let mut user_builder = UserServiceBuilder::new()
        .with_service(gateway_service.clone())
        .with_public(proxy_fqdn)
        .with_user_proxy_binding_to(args.user)
        .with_bouncer(args.bouncer);

    if let UseTls::Enable = args.use_tls {
        let (resolver, tls_acceptor) = make_tls_acceptor();

        user_builder = user_builder
            .with_acme(acme_client.clone())
            .with_tls(tls_acceptor);

        api_builder = api_builder.with_acme(acme_client.clone(), resolver.clone());

        for CustomDomain {
            fqdn,
            certificate,
            private_key,
            ..
        } in gateway_service.iter_custom_domains().await.unwrap()
        {
            let mut buf = Vec::new();
            buf.extend(certificate.as_bytes());
            buf.extend(private_key.as_bytes());
            resolver
                .serve_pem(&fqdn.to_string(), Cursor::new(buf))
                .await
                .unwrap();
        }

        tokio::spawn(async move {
            // Make sure we have a certificate for ourselves.
            let certs = gateway_service
                .fetch_certificate(&acme_client, gateway_service.credentials())
                .await;
            resolver
                .serve_default_der(certs)
                .await
                .expect("failed to set certs to be served as default");
        });
    } else {
        warn!("TLS is disabled in the proxy service. This is only acceptable in testing, and should *never* be used in deployments.");
    };

    let api_handle = api_builder
        .with_default_routes()
        .with_auth_service(&args.auth_uri)
        .await
        .with_default_traces()
        .serve();

    let user_handle = user_builder.serve();

    debug!("starting up all services");

    tokio::select!(
        _ = api_handle => error!("api handle finished"),
        _ = user_handle => error!("user handle finished"),
    );

    Ok(())
}
