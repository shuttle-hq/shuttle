use http::header::AUTHORIZATION;
use http::{Request, StatusCode};
use hyper::Body;
use serde_json::Value;
use shuttle_common::claims::{AccountTier, Claim};

use crate::helpers::app;

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
    let request = Request::builder()
        .uri("/auth/key")
        .header(AUTHORIZATION, format!("Bearer {api_key}"))
        .body(Body::empty())
        .unwrap();

    let response = app.send_request(request).await;

    assert_eq!(response.status(), StatusCode::OK);

    let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let convert: Value = serde_json::from_slice(&body).unwrap();
    let token = convert["token"].as_str().unwrap();

    let request = Request::builder()
        .uri("/public-key")
        .method("GET")
        .body(Body::empty())
        .unwrap();
    let response = app.send_request(request).await;

    assert_eq!(response.status(), StatusCode::OK);

    let public_key = hyper::body::to_bytes(response.into_body()).await.unwrap();

    let claim = Claim::from_token(token, &public_key).unwrap();

    // Verify the claim subject and tier matches the test user we created.
    assert_eq!(claim.sub, "test-user");
    assert_eq!(claim.tier, AccountTier::Basic);
}
