mod needs_docker {
    use crate::{
        helpers::{self, app, ADMIN_KEY, STRIPE_TEST_KEY, STRIPE_TEST_RDS_PRICE_ID},
        stripe::{MOCKED_CHECKOUT_SESSIONS, MOCKED_SUBSCRIPTIONS},
    };
    use axum::body::Body;
    use http::header::CONTENT_TYPE;
    use hyper::http::{header::AUTHORIZATION, Request, StatusCode};
    use serde_json::{self, Value};
    use shuttle_common::backends::subscription::NewSubscriptionItem;
    use wiremock::{
        matchers::{bearer_token, body_string_contains, method, path},
        Mock, ResponseTemplate,
    };

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
            .put_user("test-user", "pro", MOCKED_CHECKOUT_SESSIONS[0])
            .await;
        assert_eq!(response.status(), StatusCode::OK);

        // Next we're going to fetch the user, which will trigger a sync of the users tier. It will
        // fetch the subscription from stripe using the subscription ID from the previous checkout
        // session. This should return an active subscription, meaning the users tier should remain
        // pro.
        let response = app
            .get_user_with_mocked_stripe(
                "sub_1Nw8xOD8t1tt0S3DtwAuOVp6",
                MOCKED_SUBSCRIPTIONS[0],
                "test-user",
            )
            .await;

        assert_eq!(response.status(), StatusCode::OK);
        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        let actual_user: Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(
            expected_user.as_object().unwrap().get("name").unwrap(),
            actual_user.as_object().unwrap().get("name").unwrap()
        );

        assert_eq!(
            expected_user.as_object().unwrap().get("key").unwrap(),
            actual_user.as_object().unwrap().get("key").unwrap()
        );

        assert_eq!(
            actual_user
                .as_object()
                .unwrap()
                .get("account_tier")
                .unwrap(),
            "pro"
        );

        let mocked_subscription_obj: Value = serde_json::from_str(MOCKED_SUBSCRIPTIONS[0]).unwrap();
        assert_eq!(
            actual_user
                .as_object()
                .unwrap()
                .get("subscription_id")
                .unwrap(),
            mocked_subscription_obj
                .as_object()
                .unwrap()
                .get("id")
                .unwrap()
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
            .put_user("test-user", "pro", MOCKED_CHECKOUT_SESSIONS[1])
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
            .put_user("test-user", "pro", MOCKED_CHECKOUT_SESSIONS[2])
            .await;
        assert_eq!(response.status(), StatusCode::OK);

        // The auth service should call stripe to fetch the subscription with the sub id from the
        // checkout session, and return a subscription that is pending payment.
        let response = app
            .get_user_with_mocked_stripe(
                "sub_1NwObED8t1tt0S3Dq0IYOEsa",
                MOCKED_SUBSCRIPTIONS[1],
                "test-user",
            )
            .await;

        assert_eq!(response.status(), StatusCode::OK);

        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        let actual_user: Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(
            actual_user
                .as_object()
                .unwrap()
                .get("account_tier")
                .unwrap(),
            "pendingpaymentpro"
        );
    }

    #[tokio::test]
    async fn update_subscription_endpoint_requires_jwt() {
        let app = app().await;

        let subscription_item = serde_json::to_string(&NewSubscriptionItem::new(
            shuttle_common::backends::subscription::SubscriptionItem::AwsRds,
            1,
        ))
        .unwrap();

        // POST /users/subscription/items without bearer JWT.
        let request = Request::builder()
            .uri("/users/subscription/items")
            .method("POST")
            .header(CONTENT_TYPE, "application/json")
            .body(Body::from(subscription_item.clone()))
            .unwrap();

        let response = app.send_request(request).await;

        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);

        // Extract the body from the response so we can match on the error message.
        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        let message = std::str::from_utf8(&body).unwrap();
        // Since there is no bearer token, no claim extension could be set.
        assert!(message.contains("Missing request extension"));

        // POST /users/subscription/items with invalid bearer JWT.
        let request = Request::builder()
            .uri("/users/subscription/items")
            .method("POST")
            .header(CONTENT_TYPE, "application/json")
            .header(AUTHORIZATION, "invalid token")
            .body(Body::from(subscription_item.clone()))
            .unwrap();

        let response = app.send_request(request).await;

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        // GET /auth/key with the api key of the admin user to get their jwt.
        let response = app.get_jwt_from_api_key(ADMIN_KEY).await;

        assert_eq!(response.status(), StatusCode::OK);

        // Extract the token.
        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        let convert: Value = serde_json::from_slice(&body).unwrap();
        let token = convert["token"].as_str().unwrap();

        // POST /users/:account_name with valid JWT.
        let request = || {
            Request::builder()
                .uri("/users/subscription/items")
                .method("POST")
                .header(CONTENT_TYPE, "application/json")
                .header(AUTHORIZATION, format!("Bearer {}", token))
                .body(Body::from(subscription_item.clone()))
                .unwrap()
        };

        let response = app.send_request(request()).await;

        // The test user (claim subject) does not have a subscription ID.
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        // Upgrade the user to pro so they have subscription ID.
        let response = app
            .put_user("admin", "pro", MOCKED_CHECKOUT_SESSIONS[0])
            .await;

        assert_eq!(response.status(), StatusCode::OK);

        // We now want to retry the request.
        // In the process the auth service will try to sync the tier, fetching the subscription
        // from stripe.
        Mock::given(method("GET"))
            .and(bearer_token(STRIPE_TEST_KEY))
            .and(path("/v1/subscriptions/sub_1Nw8xOD8t1tt0S3DtwAuOVp6"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::from_str::<Value>(MOCKED_SUBSCRIPTIONS[0]).unwrap()),
            )
            .mount(&app.mock_server)
            .await;

        // We just return a mocked active subscription without the RDS items, our logic doesn't check
        // the subscription after updating, if it receives a 200 and a correctly formed subscription
        // response we know that the update succeeded.
        // We also want to ensure it's called with the correct price_id, the one the auth serviec was
        // started with, as well as the quantity field.
        Mock::given(method("POST"))
            .and(bearer_token(STRIPE_TEST_KEY))
            .and(path("/v1/subscriptions/sub_1Nw8xOD8t1tt0S3DtwAuOVp6"))
            .and(body_string_contains(STRIPE_TEST_RDS_PRICE_ID))
            .and(body_string_contains("quantity"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::from_str::<Value>(MOCKED_SUBSCRIPTIONS[0]).unwrap()),
            )
            .mount(&app.mock_server)
            .await;

        // POST /users/:account_name with valid JWT and the user upgraded to pro.
        let response = app.send_request(request()).await;

        assert_eq!(response.status(), StatusCode::OK);
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
            .put_user("test-user", "pro", MOCKED_CHECKOUT_SESSIONS[3])
            .await;
        assert_eq!(response.status(), StatusCode::OK);

        // Cancel subscription, this will be called by the console.
        let response = app.put_user("test-user", "cancelledpro", "").await;
        assert_eq!(response.status(), StatusCode::OK);

        // Fetch the user to trigger a sync of the account tier to cancelled. The account should not
        // be downgraded to basic right away, since when we cancel subscriptions we pass in the
        // "cancel_at_period_end" end flag.
        let response = app
            .get_user_with_mocked_stripe("sub_123", MOCKED_SUBSCRIPTIONS[2], "test-user")
            .await;

        assert_eq!(response.status(), StatusCode::OK);

        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        let user: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(
            user.as_object().unwrap().get("account_tier").unwrap(),
            "cancelledpro"
        );

        // When called again at some later time, the subscription returned from stripe should be
        // cancelled.
        let response = app
            .get_user_with_mocked_stripe("sub_123", MOCKED_SUBSCRIPTIONS[3], "test-user")
            .await;
        assert_eq!(response.status(), StatusCode::OK);

        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        let user: Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(
            user.as_object().unwrap().get("account_tier").unwrap(),
            "basic"
        );
    }
}
