use std::net::Ipv4Addr;

use axum::{
    headers::{Authorization, HeaderMapExt},
    http::Request,
    middleware::Next,
    response::Response,
    Extension,
};
use hyper::{
    client::{connect::dns::GaiResolver, HttpConnector},
    header::AUTHORIZATION,
    Body, Client, StatusCode, Uri,
};
use hyper_reverse_proxy::ReverseProxy;
use once_cell::sync::Lazy;
use serde_json::Value;

const LOCAL_ADMIN_KEY: &str = "test-key";

static PROXY_CLIENT: Lazy<ReverseProxy<HttpConnector<GaiResolver>>> =
    Lazy::new(|| ReverseProxy::new(Client::new()));

/// This middleware sends a request to the auth service with a [LOCAL_ADMIN_KEY] Bearer token, to
/// convert the [LOCAL_ADMIN_KEY] to a JWT. We extract the JWT and set it as the Bearer token of
/// the request as it proceeds into the deployer router, where it is converted to a Claim with the
/// token included. This way we can both access the admin scoped routes on deployer, and using the
/// token in the Claim we can pass the ClaimLayer in the provisioner and runtime clients when we
/// need to start services and provision resources.
///
/// Follow the steps in https://github.com/shuttle-hq/shuttle/blob/main/CONTRIBUTING.md#testing-deployer-only
/// to learn how to insert the [LOCAL_ADMIN_KEY] in the auth state.
///
/// WARNING: do not set this layer in production.
pub async fn set_jwt_bearer<B>(
    Extension(auth_uri): Extension<Uri>,
    mut request: Request<B>,
    next: Next<B>,
) -> Result<Response, StatusCode> {
    let auth_request = Request::builder()
        .uri("http://localhost:8008/auth/key")
        .header(AUTHORIZATION, format!("Bearer {LOCAL_ADMIN_KEY}"))
        .body(Body::empty())
        .unwrap();

    let response = PROXY_CLIENT
        .call(
            Ipv4Addr::LOCALHOST.into(),
            &auth_uri.to_string(),
            auth_request,
        )
        .await
        .expect("failed to proxy request to auth service");

    // Since this will only be used for local development, we can always trust the client
    // to not send a large body, so we skip the size check.
    let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let convert: Value = serde_json::from_slice(&body).unwrap();

    let token = convert["token"]
        .as_str()
        .expect("response body should have a token");

    request
        .headers_mut()
        .typed_insert(Authorization::bearer(token).expect("to set JWT token"));

    let response = next.run(request).await;

    Ok(response)
}
