use std::time::Duration;

use crate::{
    helpers::{self, app, ADMIN_KEY},
    stripe::{MOCKED_CHECKOUT_SESSIONS, MOCKED_SUBSCRIPTIONS},
};
use axum::body::Body;
use http::header::CONTENT_TYPE;
use hyper::http::{header::AUTHORIZATION, Request, StatusCode};
use serde_json::{self, Value};
use shuttle_common::backends::subscription::SubscriptionItem;

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
async fn successful_upgrade_to_pro() {
    let app = app().await;

    // Wait for the mocked Stripe server to start.
    tokio::task::spawn(app.mocked_stripe_server.clone().serve());
    tokio::time::sleep(Duration::from_secs(1)).await;

    // POST user first so one exists in the database.
    let response = app.post_user("test-user", "basic").await;

    assert_eq!(response.status(), StatusCode::OK);

    let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let expected_user: Value = serde_json::from_slice(&body).unwrap();

    let response = app
        .put_user("test-user", "pro", MOCKED_CHECKOUT_SESSIONS[0])
        .await;
    assert_eq!(response.status(), StatusCode::OK);

    let response = app.get_user("test-user").await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let actual_user: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(
        expected_user.as_object().unwrap().get("name").unwrap(),
        actual_user.as_object().unwrap().get("name").unwrap()
    );

    assert_eq!(
        expected_user.as_object().unwrap().get("key").unwrap(),
        actual_user.as_object().unwrap().get("key").unwrap()
    );

    assert_eq!(
        actual_user
            .as_object()
            .unwrap()
            .get("account_tier")
            .unwrap(),
        "pro"
    );

    let mocked_subscription_obj: Value = serde_json::from_str(MOCKED_SUBSCRIPTIONS[0]).unwrap();
    assert_eq!(
        actual_user
            .as_object()
            .unwrap()
            .get("subscription_id")
            .unwrap(),
        mocked_subscription_obj
            .as_object()
            .unwrap()
            .get("id")
            .unwrap()
    );
}

#[tokio::test]
async fn downgrade_from_cancelledpro() {
    let app = app().await;

    // Wait for the mocked Stripe server to start.
    tokio::task::spawn(app.mocked_stripe_server.clone().serve());
    tokio::time::sleep(Duration::from_secs(1)).await;

    // Create user with basic tier
    let response = app.post_user("test-user", "basic").await;
    assert_eq!(response.status(), StatusCode::OK);

    // Upgrade user to pro
    let response = app
        .put_user("test-user", "pro", MOCKED_CHECKOUT_SESSIONS[3])
        .await;
    assert_eq!(response.status(), StatusCode::OK);

    // Cancel subscription
    let response = app.put_user("test-user", "cancelledpro", "").await;
    assert_eq!(response.status(), StatusCode::OK);

    // Trigger status change to canceled
    let response = app.get_user("test-user").await;
    assert_eq!(response.status(), StatusCode::OK);

    let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let user: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(
        user.as_object().unwrap().get("account_tier").unwrap(),
        "cancelledpro"
    );

    // Check if user is downgraded to basic
    let response = app.get_user("test-user").await;
    assert_eq!(response.status(), StatusCode::OK);

    let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let user: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(
        user.as_object().unwrap().get("account_tier").unwrap(),
        "basic"
    );
}

#[tokio::test]
async fn retain_cancelledpro_status() {
    let app = app().await;

    // Wait for the mocked Stripe server to start.
    tokio::task::spawn(app.mocked_stripe_server.clone().serve());
    tokio::time::sleep(Duration::from_secs(1)).await;

    // Create user with basic tier
    let response = app.post_user("test-user", "basic").await;
    assert_eq!(response.status(), StatusCode::OK);

    // Upgrade user to pro
    let response = app
        .put_user("test-user", "pro", MOCKED_CHECKOUT_SESSIONS[3])
        .await;
    assert_eq!(response.status(), StatusCode::OK);

    // Cancel subscription
    let response = app.put_user("test-user", "cancelledpro", "").await;
    assert_eq!(response.status(), StatusCode::OK);

    // Check if user has cancelledpro status
    let response = app.get_user("test-user").await;
    assert_eq!(response.status(), StatusCode::OK);

    let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let user: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(
        user.as_object().unwrap().get("account_tier").unwrap(),
        "cancelledpro"
    );
}

#[tokio::test]
async fn unsuccessful_upgrade_to_pro() {
    let app = app().await;

    // Wait for the mocked Stripe server to start.
    tokio::task::spawn(app.mocked_stripe_server.clone().serve());
    tokio::time::sleep(Duration::from_secs(1)).await;

    // POST user first so one exists in the database.
    let response = app.post_user("test-user", "basic").await;
    assert_eq!(response.status(), StatusCode::OK);

    // Test upgrading to pro without a checkout session object.
    let response = app.put_user("test-user", "pro", "").await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    // Test upgrading to pro with an incomplete checkout session object.
    let response = app
        .put_user("test-user", "pro", MOCKED_CHECKOUT_SESSIONS[1])
        .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn downgrade_in_case_subscription_due_payment() {
    let app = app().await;

    // Wait for the mocked Stripe server to start.
    tokio::task::spawn(app.mocked_stripe_server.clone().serve());
    tokio::time::sleep(Duration::from_secs(1)).await;

    // POST user first so one exists in the database.
    let response = app.post_user("test-user", "basic").await;
    assert_eq!(response.status(), StatusCode::OK);

    // Test upgrading to pro with a checkout session that points to a due session.
    let response = app
        .put_user("test-user", "pro", MOCKED_CHECKOUT_SESSIONS[2])
        .await;
    assert_eq!(response.status(), StatusCode::OK);

    // This get_user request should check the subscription status and return an accurate tier.
    let response = app.get_user("test-user").await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let actual_user: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(
        actual_user
            .as_object()
            .unwrap()
            .get("account_tier")
            .unwrap(),
        "pendingpaymentpro"
    );
}

#[tokio::test]
async fn update_subscription_endpoint_requires_jwt() {
    let app = app().await;

    let subscription_item = serde_json::to_string(&SubscriptionItem::new(
        shuttle_common::backends::subscription::PriceId::AwsRdsRecurring,
        1,
    ))
    .unwrap();

    // POST /users/subscription/items without bearer JWT.
    let request = Request::builder()
        .uri("/users/subscription/items")
        .method("POST")
        .header(CONTENT_TYPE, "application/json")
        .body(Body::from(subscription_item.clone()))
        .unwrap();

    let response = app.send_request(request).await;

    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);

    // Extract the body from the response so we can match on the error message.
    let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let message = std::str::from_utf8(&body).unwrap();

    // Since there is no bearer token, no claim extension could be set.
    assert!(message.contains("Missing request extension"));

    // POST /users/subscription/items with invalid bearer JWT.
    let request = Request::builder()
        .uri("/users/subscription/items")
        .method("POST")
        .header(CONTENT_TYPE, "application/json")
        .header(AUTHORIZATION, "invalid token")
        .body(Body::from(subscription_item.clone()))
        .unwrap();

    let response = app.send_request(request).await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    // GET /auth/key with the api key of the admin user to get their jwt.
    let response = app.get_jwt_from_api_key(ADMIN_KEY).await;

    assert_eq!(response.status(), StatusCode::OK);

    // Extract the token.
    let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let convert: Value = serde_json::from_slice(&body).unwrap();
    let token = convert["token"].as_str().unwrap();

    // POST /users/:account_name with valid JWT.
    let request = Request::builder()
        .uri("/users/subscription/items")
        .method("POST")
        .header(CONTENT_TYPE, "application/json")
        .header(AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::from(subscription_item))
        .unwrap();

    let response = app.send_request(request).await;

    // TODO: The request is valid, but the endpoint is not able to correctly call stripe
    // in this test, so for now it returns 500.
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn test_reset_key() {
    let app = app().await;

    // Reset API key without API key.
    let request = Request::builder()
        .uri("/users/reset-api-key")
        .method("PUT")
        .body(Body::empty())
        .unwrap();
    let response = app.send_request(request).await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

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
