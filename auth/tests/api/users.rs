use crate::helpers::spawn_app;
use serde_json::{self, Value};

#[tokio::test]
async fn post_user() {
    let app = spawn_app().await;

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

    let response = app
        .api_client
        .get(format!("{}/user/test-user", app.address))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status().as_u16(), 200);

    let persisted_user: Value = serde_json::from_slice(&response.bytes().await.unwrap()).unwrap();

    assert_eq!(post_user, persisted_user);
}
