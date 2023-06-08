use pretty_assertions::{assert_eq, assert_ne};
use shuttle_proto::auth::{ApiKeyRequest, NewUser, UserRequest, UserResponse};
use tonic::{metadata::MetadataValue, Code, Request};

use crate::helpers::spawn_app;

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

    // First create a new user who's key we can reset.
    let UserResponse { key, .. } = app
        .post_user("basic-user", "basic")
        .await
        .unwrap()
        .into_inner();

    // Reset API key with api key from the user we created.
    let request = Request::new(ApiKeyRequest {
        api_key: key.clone(),
    });

    let response = app
        .client
        .reset_api_key(request)
        .await
        .unwrap()
        .into_inner();

    assert!(response.success);

    // GET the new user to verify it's api key changed.
    let response = app.get_user("basic-user").await.unwrap().into_inner();

    assert_ne!(key, response.key);
}
