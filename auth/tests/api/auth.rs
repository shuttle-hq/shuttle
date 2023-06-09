use crate::helpers::spawn_app;
use pretty_assertions::assert_eq;
use shuttle_common::claims::Claim;
use shuttle_proto::auth::{ApiKeyRequest, PublicKeyRequest, UserResponse};
use tonic::Request;

#[tokio::test]
async fn convert_api_key_to_jwt_and_decode_jwt() {
    let mut app = spawn_app().await;

    // Create test user.
    let UserResponse {
        key, account_name, ..
    } = app
        .post_user("basic-user", "basic")
        .await
        .unwrap()
        .into_inner();

    // Create a request to convert the API-key for the user we just created.
    let request = Request::new(ApiKeyRequest { api_key: key });

    // Send convert request.
    let response = app
        .client
        .convert_api_key(request)
        .await
        .unwrap()
        .into_inner();

    let token = response.token;

    // We need to get the public key to decode the JWT.
    let request = Request::new(PublicKeyRequest {});

    let response = app.client.public_key(request).await.unwrap().into_inner();

    let claim = Claim::from_token(&token, &response.public_key).unwrap();

    assert_eq!(account_name, claim.sub);
}
