use shuttle_proto::auth::{NewUser, UserRequest};
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

// #[tokio::test]
// async fn test_reset_key() {
//     let app = app().await;

//     // Reset API key without cookie or API key.
//     let request = Request::builder()
//         .uri("/users/reset-api-key")
//         .method("PUT")
//         .body(Body::empty())
//         .unwrap();
//     let response = app.send_request(request).await;
//     assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

//     // Reset API key with cookie.
//     let response = app.post_user("test-user", "basic").await;
//     assert_eq!(response.status(), StatusCode::OK);

//     let body = serde_json::to_vec(&json! ({"account_name": "test-user"})).unwrap();
//     let request = Request::builder()
//         .uri("/login")
//         .method("POST")
//         .header("Content-Type", "application/json")
//         .body(Body::from(body))
//         .unwrap();
//     let response = app.send_request(request).await;
//     assert_eq!(response.status(), StatusCode::OK);
//     let cookie = response
//         .headers()
//         .get("set-cookie")
//         .unwrap()
//         .to_str()
//         .unwrap();
//     let cookie = Cookie::parse(cookie).unwrap();

//     let request = Request::builder()
//         .uri("/users/reset-api-key")
//         .method("PUT")
//         .header("Cookie", cookie.stripped().to_string())
//         .body(Body::empty())
//         .unwrap();
//     let response = app.send_request(request).await;
//     assert_eq!(response.status(), StatusCode::OK);

//     // Reset API key with API key.
//     let request = Request::builder()
//         .uri("/users/reset-api-key")
//         .method("PUT")
//         .header(AUTHORIZATION, format!("Bearer {}", helpers::ADMIN_KEY))
//         .body(Body::empty())
//         .unwrap();
//     let response = app.send_request(request).await;
//     assert_eq!(response.status(), StatusCode::OK);
// }
