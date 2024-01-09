mod needs_docker {
    use crate::{
        helpers::{self, app},
        stripe::{
            MOCKED_ACTIVE_SUBSCRIPTION, MOCKED_CANCELLEDPRO_CHECKOUT_SESSION,
            MOCKED_CANCELLEDPRO_SUBSCRIPTION_ACTIVE, MOCKED_CANCELLEDPRO_SUBSCRIPTION_CANCELLED,
            MOCKED_COMPLETED_CHECKOUT_SESSION, MOCKED_INCOMPLETE_CHECKOUT_SESSION,
            MOCKED_OVERDUE_PAYMENT_CHECKOUT_SESSION, MOCKED_PAST_DUE_SUBSCRIPTION,
        },
    };
    use axum::body::Body;
    use hyper::http::{header::AUTHORIZATION, Request, StatusCode};
    use serde_json::{self, Value};

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

        // POST user first so one exists in the database.
        let response = app.post_user("test-user", "basic").await;

        assert_eq!(response.status(), StatusCode::OK);

        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        let expected_user: Value = serde_json::from_slice(&body).unwrap();

        // PUT /users/test-user/pro with a completed checkout session to upgrade a user to pro.
        let response = app
            .put_user("test-user", "pro", MOCKED_COMPLETED_CHECKOUT_SESSION)
            .await;
        assert_eq!(response.status(), StatusCode::OK);

        // Next we're going to fetch the user, which will trigger a sync of the users tier. It will
        // fetch the subscription from stripe using the subscription ID from the previous checkout
        // session. This should return an active subscription, meaning the users tier should remain
        // pro.
        let response = app
            .get_user_with_mocked_stripe(
                "sub_1Nw8xOD8t1tt0S3DtwAuOVp6",
                MOCKED_ACTIVE_SUBSCRIPTION,
                "test-user",
            )
            .await;

        assert_eq!(response.status(), StatusCode::OK);
        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        let actual_user: Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(
            expected_user.get("name").unwrap(),
            actual_user.get("name").unwrap()
        );

        assert_eq!(
            expected_user.get("key").unwrap(),
            actual_user.get("key").unwrap()
        );

        assert_eq!(actual_user.get("account_tier").unwrap(), "pro");

        let mocked_subscription_obj: Value =
            serde_json::from_str(MOCKED_ACTIVE_SUBSCRIPTION).unwrap();
        assert_eq!(
            actual_user.get("subscription_id").unwrap(),
            mocked_subscription_obj.get("id").unwrap()
        );
    }

    #[tokio::test]
    async fn unsuccessful_upgrade_to_pro() {
        let app = app().await;

        // POST user first so one exists in the database.
        let response = app.post_user("test-user", "basic").await;
        assert_eq!(response.status(), StatusCode::OK);

        // Test upgrading to pro without a checkout session object.
        let response = app.put_user("test-user", "pro", "").await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        // Test upgrading to pro with an incomplete checkout session object.
        let response = app
            .put_user("test-user", "pro", MOCKED_INCOMPLETE_CHECKOUT_SESSION)
            .await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn downgrade_in_case_subscription_due_payment() {
        let app = app().await;

        // POST user first so one exists in the database.
        let response = app.post_user("test-user", "basic").await;
        assert_eq!(response.status(), StatusCode::OK);

        // Test upgrading to pro with a checkout session that points to a due session.
        let response = app
            .put_user("test-user", "pro", MOCKED_OVERDUE_PAYMENT_CHECKOUT_SESSION)
            .await;
        assert_eq!(response.status(), StatusCode::OK);

        // The auth service should call stripe to fetch the subscription with the sub id from the
        // checkout session, and return a subscription that is pending payment.
        let response = app
            .get_user_with_mocked_stripe(
                "sub_1NwObED8t1tt0S3Dq0IYOEsa",
                MOCKED_PAST_DUE_SUBSCRIPTION,
                "test-user",
            )
            .await;

        assert_eq!(response.status(), StatusCode::OK);

        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        let actual_user: Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(
            actual_user.get("account_tier").unwrap(),
            "pendingpaymentpro"
        );
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

    #[tokio::test]
    async fn downgrade_from_cancelledpro() {
        let app = app().await;

        // Create user with basic tier
        let response = app.post_user("test-user", "basic").await;
        assert_eq!(response.status(), StatusCode::OK);

        // Upgrade user to pro
        let response = app
            .put_user("test-user", "pro", MOCKED_CANCELLEDPRO_CHECKOUT_SESSION)
            .await;
        assert_eq!(response.status(), StatusCode::OK);

        // Cancel subscription, this will be called by the console.
        let response = app.put_user("test-user", "cancelledpro", "").await;
        assert_eq!(response.status(), StatusCode::OK);

        // Fetch the user to trigger a sync of the account tier to cancelled. The account should not
        // be downgraded to basic right away, since when we cancel subscriptions we pass in the
        // "cancel_at_period_end" end flag.
        let response = app
            .get_user_with_mocked_stripe(
                "sub_123",
                MOCKED_CANCELLEDPRO_SUBSCRIPTION_ACTIVE,
                "test-user",
            )
            .await;

        assert_eq!(response.status(), StatusCode::OK);

        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        let user: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(user.get("account_tier").unwrap(), "cancelledpro");

        // When called again at some later time, the subscription returned from stripe should be
        // cancelled.
        let response = app
            .get_user_with_mocked_stripe(
                "sub_123",
                MOCKED_CANCELLEDPRO_SUBSCRIPTION_CANCELLED,
                "test-user",
            )
            .await;
        assert_eq!(response.status(), StatusCode::OK);

        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        let user: Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(user.get("account_tier").unwrap(), "basic");
    }
}
