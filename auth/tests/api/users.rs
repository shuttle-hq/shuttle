use cookie::Cookie;
use http::header::COOKIE;
use pretty_assertions::{assert_eq, assert_ne};
use shuttle_proto::auth::{NewUser, ResetKeyRequest, UserRequest, UserResponse};
use tonic::{metadata::MetadataValue, Code, Request};

use crate::helpers::{spawn_app, ADMIN_KEY};

#[tokio::test]
async fn post_user() {
    let mut app = spawn_app().await;

    // POST user without admin bearer token.
    let request = || {
        Request::new(NewUser {
            account_name: "basic-user".to_string(),
            account_tier: "basic".to_string(),
        })
    };

    let response = app.client.post_user_request(request()).await.err();

    assert_eq!(response.unwrap().code(), Code::PermissionDenied);

    // POST user with invalid admin bearer token.
    let mut request = request();
    let bearer: MetadataValue<_> = ("Bearer notadmintoken123").parse().unwrap();
    request.metadata_mut().insert("authorization", bearer);

    let response = app.client.post_user_request(request).await.err();

    assert_eq!(response.unwrap().code(), Code::PermissionDenied);

    // POST user with valid admin bearer token and basic tier.
    let response = app
        .post_user("basic-user", "basic")
        .await
        .unwrap()
        .into_inner();

    assert_eq!(response.account_name, "basic-user".to_string());
    assert_eq!(response.account_tier, "basic".to_string());

    // POST user with valid admin bearer token and pro tier.
    let response = app.post_user("pro-user", "pro").await.unwrap().into_inner();

    assert_eq!(response.account_name, "pro-user".to_string());
    assert_eq!(response.account_tier, "pro".to_string());
}

#[tokio::test]
async fn get_user() {
    let mut app = spawn_app().await;

    // POST user first so one exists in the database.
    let persisted_user = app
        .post_user("test-user", "basic")
        .await
        .unwrap()
        .into_inner();

    // GET user without bearer token.
    let request = || {
        Request::new(UserRequest {
            account_name: "test-user".to_string(),
        })
    };

    let response = app.client.get_user_request(request()).await.err();

    assert_eq!(response.unwrap().code(), Code::PermissionDenied);

    // GET user with invalid bearer token.
    let mut request = request();
    let bearer: MetadataValue<_> = ("Bearer notadmintoken123").parse().unwrap();
    request.metadata_mut().insert("authorization", bearer);

    let response = app.client.get_user_request(request).await.err();

    assert_eq!(response.unwrap().code(), Code::PermissionDenied);

    // GET user that doesn't exist with valid bearer token.
    let response = app.get_user("not-test-user").await.err();

    assert_eq!(response.unwrap().code(), Code::NotFound);

    // GET user with valid bearer token.
    let response = app.get_user("test-user").await;

    assert_eq!(response.unwrap().into_inner(), persisted_user);
}

#[tokio::test]
async fn test_reset_key() {
    let mut app = spawn_app().await;

    const RESET_KEY_USER: &str = "basic-user";

    // First create a new user who's key we can reset.
    let UserResponse {
        key, account_name, ..
    } = app
        .post_user(RESET_KEY_USER, "basic")
        .await
        .unwrap()
        .into_inner();

    // Reset API key with api key from the user we created.
    let request = Request::new(ResetKeyRequest {
        api_key: Some(key.clone()),
    });

    let response = app
        .client
        .reset_api_key(request)
        .await
        .unwrap()
        .into_inner();

    assert!(response.success);

    // Get the user again to verify it's api key changed.
    let UserResponse { key: new_key, .. } =
        app.get_user(RESET_KEY_USER).await.unwrap().into_inner();

    assert_ne!(key, new_key);

    // Test resetting a key with a cookie, first login our user.
    let mut request = Request::new(UserRequest { account_name });

    let bearer: MetadataValue<_> = format!("Bearer {ADMIN_KEY}").parse().unwrap();
    request.metadata_mut().insert("authorization", bearer);

    let response = app.client.login(request).await.unwrap();

    let cookie = response
        .metadata()
        .get("set-cookie")
        .unwrap()
        .to_str()
        .unwrap();

    let cookie = Cookie::parse(cookie).unwrap();

    // Then create our reset request and send it without the cookie or api-key.
    let request = Request::new(ResetKeyRequest::default());

    // Reset our api-key
    let response = app
        .client
        .reset_api_key(request)
        .await
        .unwrap()
        .into_inner();

    assert!(!response.success);

    // Repeat the above but now with the cookie inserted.
    let mut request = Request::new(ResetKeyRequest::default());

    request.metadata_mut().insert(
        COOKIE.as_str(),
        MetadataValue::try_from(&cookie.to_string())
            .expect("cookie should not contain invalid metadata value characters"),
    );

    // Reset our api-key
    let response = app
        .client
        .reset_api_key(request)
        .await
        .unwrap()
        .into_inner();

    assert!(response.success);

    // Get the user again to verify it's api key changed.
    let response = app.get_user(RESET_KEY_USER).await.unwrap().into_inner();

    assert_ne!(new_key, response.key);
}
