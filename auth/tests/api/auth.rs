mod needs_docker {
    use http::header::AUTHORIZATION;
    use http::{Request, StatusCode};
    use hyper::Body;
    use serde_json::Value;
    use shuttle_common::models::user::AccountTier;

    use crate::helpers::{app, ADMIN_KEY};

    #[tokio::test]
    async fn convert_api_key_to_jwt() {
        let app = app().await;

        // Create test basic user
        let response = app.post_user("test-user-basic", "basic").await;
        assert_eq!(response.status(), StatusCode::OK);
        // Extract the API key from the response so we can use it in a future request.
        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        let user: Value = serde_json::from_slice(&body).unwrap();
        let basic_user_key = user["key"].as_str().unwrap();

        // GET /auth/key without bearer token.
        let request = Request::builder()
            .uri("/auth/key")
            .body(Body::empty())
            .unwrap();
        let response = app.send_request(request).await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        // GET /auth/key with basic tier user API key.
        let request = Request::builder()
            .uri("/auth/key")
            .header(AUTHORIZATION, format!("Bearer {basic_user_key}"))
            .body(Body::empty())
            .unwrap();
        let response = app.send_request(request).await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        // GET /auth/key with an admin user key.
        let response = app.get_jwt_from_api_key(ADMIN_KEY, None).await;
        assert_eq!(response.status(), StatusCode::OK);

        // Decode the JWT into a Claim.
        let claim = app.claim_from_response(response).await;

        // Verify the claim subject and tier matches the test user we created at the start of the test.
        assert!(claim.sub.starts_with("user_"));
        assert_eq!(claim.tier, AccountTier::Admin);
        assert_eq!(claim.limits.project_limit(), 3);

        // GET /auth/key with a basic user key that has an XShuttleAdminSecret header with a basic user key.
        let response = app
            .get_jwt_from_api_key(basic_user_key, Some(basic_user_key))
            .await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        // GET /auth/key with an admin user key that has an XShuttleAdminSecret header with a basic user key.
        let response = app
            .get_jwt_from_api_key(ADMIN_KEY, Some(basic_user_key))
            .await;
        assert_eq!(response.status(), StatusCode::OK);

        // Decode the JWT into a Claim.
        let claim = app.claim_from_response(response).await;

        // Verify the claim subject and tier matches the test user we created at the start of the test.
        assert!(claim.sub.starts_with("user_"));
        assert_eq!(claim.tier, AccountTier::Admin);
        assert_eq!(claim.limits.project_limit(), 3);

        // GET /auth/key with a basic user key that has an XShuttleAdminSecret header with an admin user key.
        let response = app
            .get_jwt_from_api_key(basic_user_key, Some(ADMIN_KEY))
            .await;
        assert_eq!(response.status(), StatusCode::OK);

        // Decode the JWT into a Claim.
        let claim = app.claim_from_response(response).await;

        // Verify the claim subject and tier matches the admin user.
        assert!(claim.sub.starts_with("user_"));
        assert_eq!(claim.tier, AccountTier::Basic);
        assert_eq!(claim.limits.project_limit(), 3);
    }
}
