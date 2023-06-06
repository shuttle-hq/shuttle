use http::header::AUTHORIZATION;
use http::{Request, StatusCode};
use hyper::Body;

use crate::helpers::{app, ADMIN_KEY};

#[tokio::test]
async fn convert_api_key_to_jwt() {
    let app = app().await;

    // Create test user
    let response = app.post_user("test-user", "basic").await;

    assert_eq!(response.status(), StatusCode::OK);

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
        .header(AUTHORIZATION, format!("Bearer {ADMIN_KEY}"))
        .body(Body::empty())
        .unwrap();

    let response = app.send_request(request).await;

    assert_eq!(response.status(), StatusCode::OK);

    // TODO: decode the JWT?
}
