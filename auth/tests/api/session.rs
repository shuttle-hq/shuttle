use axum_extra::extract::cookie::{self, Cookie};
use http::{Request, StatusCode};
use hyper::Body;
use serde_json::json;

use crate::helpers::app;

#[tokio::test]
async fn session_flow() {
    let app = app().await;

    // Create test user
    let response = app.post_user("session-user", "basic").await;

    assert_eq!(response.status(), StatusCode::OK);

    // POST user login
    let body = serde_json::to_vec(&json! ({"account_name": "session-user"})).unwrap();
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

    assert_eq!(cookie.http_only(), Some(true));
    assert_eq!(cookie.same_site(), Some(cookie::SameSite::Strict));
    assert_eq!(cookie.secure(), Some(true));

    // Test converting the cookie to a JWT
    let request = Request::builder()
        .uri("/auth/session")
        .method("GET")
        .header("Cookie", cookie.stripped().to_string())
        .body(Body::empty())
        .unwrap();
    let response = app.send_request(request).await;

    assert_eq!(response.status(), StatusCode::OK);

    // POST user logout
    let request = Request::builder()
        .uri("/logout")
        .method("POST")
        .header("Cookie", cookie.stripped().to_string())
        .body(Body::empty())
        .unwrap();
    let response = app.send_request(request).await;

    assert_eq!(response.status(), StatusCode::OK);

    // Test cookie can no longer be converted to JWT
    let request = Request::builder()
        .uri("/auth/session")
        .method("GET")
        .header("Cookie", cookie.stripped().to_string())
        .body(Body::empty())
        .unwrap();
    let response = app.send_request(request).await;

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}
