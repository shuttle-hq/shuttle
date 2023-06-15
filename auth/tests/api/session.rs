use cookie::{self, Cookie};
use http::header::COOKIE;
use shuttle_common::claims::Claim;
use shuttle_proto::auth::{
    ConvertCookieRequest, LogoutRequest, PublicKeyRequest, UserRequest, UserResponse,
};
use tonic::{metadata::MetadataValue, Code, Request};

use crate::helpers::spawn_app;

#[tokio::test]
async fn session_flow() {
    let mut app = spawn_app().await;

    // Create test user.
    let UserResponse { account_name, .. } = app
        .post_user("session-user", "basic")
        .await
        .unwrap()
        .into_inner();

    // Login test user.
    let request = Request::new(UserRequest {
        account_name: account_name.clone(),
    });

    let response = app.client.login(request).await.unwrap();

    let cookie = response
        .metadata()
        .get("set-cookie")
        .unwrap()
        .to_str()
        .unwrap();

    let cookie = Cookie::parse(cookie).unwrap();

    assert_eq!(cookie.http_only(), Some(true));
    assert_eq!(cookie.same_site(), Some(cookie::SameSite::Strict));
    assert_eq!(cookie.secure(), Some(true));

    // Test converting the cookie to a JWT
    let mut request = Request::new(ConvertCookieRequest {});

    request.metadata_mut().insert(
        COOKIE.as_str(),
        MetadataValue::try_from(&cookie.to_string()).unwrap(),
    );

    let response = app
        .client
        .convert_cookie(request)
        .await
        .unwrap()
        .into_inner();

    let token = response.token;

    // We need to get the public key to decode the JWT.
    let request = Request::new(PublicKeyRequest {});

    let response = app.client.public_key(request).await.unwrap().into_inner();

    let claim = Claim::from_token(&token, &response.public_key).unwrap();

    assert_eq!(account_name, claim.sub);

    // Logout our user.
    let mut request = Request::new(LogoutRequest::default());

    request.metadata_mut().insert(
        COOKIE.as_str(),
        MetadataValue::try_from(&cookie.to_string()).unwrap(),
    );

    let response = app.client.logout(request).await.unwrap();

    let logout_cookie = response
        .metadata()
        .get("set-cookie")
        .unwrap()
        .to_str()
        .unwrap();

    let logout_cookie = Cookie::parse(logout_cookie).unwrap();

    assert_eq!(logout_cookie.http_only(), Some(true));
    assert!(logout_cookie.max_age().unwrap().is_zero());

    // Try to convert the original cookie to JWT again.
    let mut request = Request::new(ConvertCookieRequest {});

    request.metadata_mut().insert(
        COOKIE.as_str(),
        MetadataValue::try_from(&cookie.to_string()).unwrap(),
    );

    let response = app.client.convert_cookie(request).await;

    assert_eq!(response.err().unwrap().code(), Code::PermissionDenied);
}
