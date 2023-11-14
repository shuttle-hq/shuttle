use http::header::AUTHORIZATION;
use http::{Request, StatusCode};
use hyper::Body;
use serde_json::Value;
use shuttle_common::claims::{AccountTier, ClaimExt};

use crate::helpers::{app, ADMIN_KEY};

#[tokio::test]
async fn convert_api_key_to_jwt() {
    let app = app().await;

    // Create test user
    let response = app.post_user("test-user", "basic").await;

    assert_eq!(response.status(), StatusCode::OK);

    // Extract the API key from the response so we can use it in a future request.
    let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let user: Value = serde_json::from_slice(&body).unwrap();
    let api_key = user["key"].as_str().unwrap();

    // GET /auth/key without bearer token.
    let request = Request::builder()
        .uri("/auth/key")
        .body(Body::empty())
        .unwrap();

    let response = app.send_request(request).await;

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    // GET /auth/key with invalid bearer token.
    let request = Request::builder()
        .uri("/auth/key")
        .header(AUTHORIZATION, "Bearer ndh9z58jttoefake")
        .body(Body::empty())
        .unwrap();

    let response = app.send_request(request).await;

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    // GET /auth/key with valid bearer token.
    let response = app.get_jwt_from_api_key(api_key).await;
    assert_eq!(response.status(), StatusCode::OK);

    // Decode the JWT into a Claim.
    let claim = app.claim_from_response(response).await;

    // Verify the claim subject and tier matches the test user we created at the start of the test.
    assert_eq!(claim.sub, "test-user");
    assert_eq!(claim.tier, AccountTier::Basic);
    assert_eq!(claim.project_limit(), 3);

    // GET /auth/key with an admin user bearer token.
    let response = app.get_jwt_from_api_key(ADMIN_KEY).await;
    assert_eq!(response.status(), StatusCode::OK);

    let claim = app.claim_from_response(response).await;

    // Verify the claim subject and tier matches the admin user.
    assert_eq!(claim.sub, "admin");
    assert_eq!(claim.tier, AccountTier::Admin);
    assert_eq!(claim.project_limit(), 100);
}
