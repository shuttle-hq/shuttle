use crate::helpers::{spawn_app, ADMIN_KEY};
use serde_json::{self, Value};

#[tokio::test]
async fn post_user() {
    let app = spawn_app().await;

    // POST user without bearer token.
    let response = app
        .api_client
        .post(&format!("{}/user/{}", app.address, "test-user"))
        .send()
        .await
        .expect("Failed to execute request.");

    assert_eq!(response.status().as_u16(), 400);

    // POST user with invalid bearer token.
    let response = app
        .api_client
        .post(&format!("{}/user/{}", app.address, "test-user"))
        .bearer_auth("notadmin")
        .send()
        .await
        .expect("Failed to execute request.");

    assert_eq!(response.status().as_u16(), 401);

    // POST user with valid bearer token.
    let response = app.post_user("test-user").await;

    assert_eq!(response.status().as_u16(), 200);

    let user: Value = serde_json::from_slice(&response.bytes().await.unwrap()).unwrap();

    assert_eq!(user["name"], "test-user");
    assert!(user["key"].to_string().is_ascii());
}

#[tokio::test]
async fn get_user() {
    let app = spawn_app().await;

    let response = app.post_user("test-user").await;

    assert_eq!(response.status().as_u16(), 200);

    let post_user: Value = serde_json::from_slice(&response.bytes().await.unwrap()).unwrap();

    // GET user without bearer token.
    let response = app
        .api_client
        .get(format!("{}/user/test-user", app.address))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status().as_u16(), 400);

    // GET user with invalid bearer token.
    let response = app
        .api_client
        .get(format!("{}/user/test-user", app.address))
        .bearer_auth("notadmin")
        .send()
        .await
        .unwrap();

    assert_eq!(response.status().as_u16(), 401);

    // GET user that doesn't exist with valid bearer token.
    let response = app
        .api_client
        .get(format!("{}/user/not-user", app.address))
        .bearer_auth(ADMIN_KEY)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status().as_u16(), 404);

    // GET user with valid bearer token.
    let response = app
        .api_client
        .get(format!("{}/user/test-user", app.address))
        .bearer_auth(ADMIN_KEY)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status().as_u16(), 200);

    let persisted_user: Value = serde_json::from_slice(&response.bytes().await.unwrap()).unwrap();

    assert_eq!(post_user, persisted_user);
}
