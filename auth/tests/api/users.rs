use crate::helpers::{self, app};
use axum::body::Body;
use axum_extra::extract::cookie::Cookie;
use hyper::http::{header::AUTHORIZATION, Request, StatusCode};
use serde_json::{self, json, Value};

#[tokio::test]
async fn post_user() {
    let app = app().await;

    // POST user without bearer token.
    let request = Request::builder()
        .uri("/users/test-user/basic")
        .method("POST")
        .body(Body::empty())
        .unwrap();

    let response = app.send_request(request).await;

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    // POST user with invalid bearer token.
    let request = Request::builder()
        .uri("/users/test-user/basic")
        .method("POST")
        .header(AUTHORIZATION, "Bearer notadmin")
        .body(Body::empty())
        .unwrap();

    let response = app.send_request(request).await;

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    // POST user with valid bearer token and basic tier.
    let response = app.post_user("test-user", "basic").await;

    assert_eq!(response.status(), StatusCode::OK);

    let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let user: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(user["name"], "test-user");
    assert_eq!(user["account_tier"], "basic");
    assert!(user["key"].to_string().is_ascii());

    // POST user with valid bearer token and pro tier.
    let response = app.post_user("pro-user", "pro").await;

    assert_eq!(response.status(), StatusCode::OK);

    let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let user: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(user["name"], "pro-user");
    assert_eq!(user["account_tier"], "pro");
    assert!(user["key"].to_string().is_ascii());
}

#[tokio::test]
async fn get_user() {
    let app = app().await;

    // POST user first so one exists in the database.
    let response = app.post_user("test-user", "basic").await;

    assert_eq!(response.status(), StatusCode::OK);

    let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let user: Value = serde_json::from_slice(&body).unwrap();

    // GET user without bearer token.
    let request = Request::builder()
        .uri("/users/test-user")
        .body(Body::empty())
        .unwrap();

    let response = app.send_request(request).await;

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    // GET user with invalid bearer token.
    let request = Request::builder()
        .uri("/users/test-user")
        .header(AUTHORIZATION, "Bearer notadmin")
        .body(Body::empty())
        .unwrap();

    let response = app.send_request(request).await;

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    // GET user that doesn't exist with valid bearer token.
    let response = app.get_user("not-test-user").await;

    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    // GET user with valid bearer token.
    let response = app.get_user("test-user").await;

    assert_eq!(response.status(), StatusCode::OK);

    let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let persisted_user: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(user, persisted_user);
}

#[tokio::test]
async fn test_reset_key() {
    let app = app().await;

    // Reset API key without cookie or API key.
    let request = Request::builder()
        .uri("/users/reset-api-key")
        .method("PUT")
        .body(Body::empty())
        .unwrap();
    let response = app.send_request(request).await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    // Reset API key with cookie.
    let response = app.post_user("test-user", "basic").await;
    assert_eq!(response.status(), StatusCode::OK);

    let body = serde_json::to_vec(&json! ({"account_name": "test-user"})).unwrap();
    let request = Request::builder()
        .uri("/login")
        .method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(body))
        .unwrap();
    let response = app.send_request(request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let cookie = response
        .headers()
        .get("set-cookie")
        .unwrap()
        .to_str()
        .unwrap();
    let cookie = Cookie::parse(cookie).unwrap();

    let request = Request::builder()
        .uri("/users/reset-api-key")
        .method("PUT")
        .header("Cookie", cookie.stripped().to_string())
        .body(Body::empty())
        .unwrap();
    let response = app.send_request(request).await;
    assert_eq!(response.status(), StatusCode::OK);

    // Reset API key with API key.
    let request = Request::builder()
        .uri("/users/reset-api-key")
        .method("PUT")
        .header(AUTHORIZATION, format!("Bearer {}", helpers::ADMIN_KEY))
        .body(Body::empty())
        .unwrap();
    let response = app.send_request(request).await;
    assert_eq!(response.status(), StatusCode::OK);
}
