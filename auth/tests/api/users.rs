mod needs_docker {
    use crate::{
        helpers::{self, app},
        stripe::{
            MOCKED_ACTIVE_SUBSCRIPTION, MOCKED_CANCELLEDPRO_CHECKOUT_SUBSCRIPTION_ID,
            MOCKED_CANCELLEDPRO_SUBSCRIPTION_ACTIVE, MOCKED_CANCELLEDPRO_SUBSCRIPTION_CANCELLED,
            MOCKED_COMPLETED_CHECKOUT_SUBSCRIPTION_ID,
            MOCKED_OVERDUE_PAYMENT_CHECKOUT_SUBSCRIPTION_ID, MOCKED_PAST_DUE_SUBSCRIPTION,
        },
    };
    use axum::body::Body;
    use hyper::http::{header::AUTHORIZATION, Request, StatusCode};
    use pretty_assertions::assert_eq;
    use serde_json::{self, Value};
    use shuttle_common::models::user::{self, AccountTier};

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
        let user: user::UserResponse = serde_json::from_slice(&body).unwrap();
        let user_id1 = user.id.clone();

        assert_eq!(user.name, "test-user");
        assert_eq!(user.account_tier, AccountTier::Basic);
        assert!(user.id.starts_with("user_"));
        assert!(user.key.is_ascii());

        // POST user with valid bearer token and pro tier.
        let response = app.post_user("pro-user", "pro").await;

        assert_eq!(response.status(), StatusCode::OK);

        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        let user: user::UserResponse = serde_json::from_slice(&body).unwrap();
        let user_id2 = user.id.clone();

        assert_eq!(user.name, "pro-user");
        assert_eq!(user.account_tier, AccountTier::Pro);
        assert!(user.id.starts_with("user_"));
        assert!(user.key.is_ascii());

        assert_eq!(
            *app.permissions.calls.lock().await,
            [
                format!("new_user {user_id1}"),
                format!("new_user {user_id2}"),
                format!("make_pro {user_id2}"),
            ]
        );
    }

    #[tokio::test]
    async fn get_user() {
        let app = app().await;

        // POST user first so one exists in the database.
        let response = app.post_user("test-user", "basic").await;

        assert_eq!(response.status(), StatusCode::OK);

        let post_body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        let user: user::UserResponse = serde_json::from_slice(&post_body).unwrap();
        let user_id = user.id;

        // GET user without bearer token.
        let request = Request::builder()
            .uri(format!("/users/{user_id}"))
            .body(Body::empty())
            .unwrap();

        let response = app.send_request(request).await;

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        // GET user with invalid bearer token.
        let request = Request::builder()
            .uri(format!("/users/{user_id}"))
            .header(AUTHORIZATION, "Bearer notadmin")
            .body(Body::empty())
            .unwrap();

        let response = app.send_request(request).await;

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        // GET user that doesn't exist with valid bearer token.
        let response = app.get_user("not-test-user").await;

        assert_eq!(response.status(), StatusCode::NOT_FOUND);

        // GET user with valid bearer token.
        let response = app.get_user(&user_id).await;

        assert_eq!(response.status(), StatusCode::OK);

        let get_body = hyper::body::to_bytes(response.into_body()).await.unwrap();

        assert_eq!(post_body, get_body);
    }

    #[tokio::test]
    async fn successful_upgrade_to_pro() {
        let app = app().await;

        // POST user first so one exists in the database.
        let response = app.post_user("test-user", "basic").await;

        assert_eq!(response.status(), StatusCode::OK);

        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        let user: user::UserResponse = serde_json::from_slice(&body).unwrap();
        let user_id = &user.id;

        // PUT /users/test-user/pro with a completed subscription id to upgrade a user to pro.
        let response = app
            .post_subscription(user_id, MOCKED_COMPLETED_CHECKOUT_SUBSCRIPTION_ID, "pro", 1)
            .await;
        assert_eq!(response.status(), StatusCode::OK);

        // Next we're going to fetch the user, which will trigger a sync of the users tier. It will
        // fetch the subscription from stripe using the previous subscription ID. This should return
        // an active subscription, meaning the users tier should remain
        // pro.
        let response = app
            .get_user_with_mocked_stripe(
                MOCKED_COMPLETED_CHECKOUT_SUBSCRIPTION_ID,
                MOCKED_ACTIVE_SUBSCRIPTION,
                user_id,
            )
            .await;

        assert_eq!(response.status(), StatusCode::OK);
        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        let pro_user: user::UserResponse = serde_json::from_slice(&body).unwrap();

        assert_eq!(user.name, pro_user.name);
        assert_eq!(user.key, pro_user.key);
        assert_eq!(pro_user.account_tier, AccountTier::Pro);

        let mocked_subscription_obj: Value =
            serde_json::from_str(MOCKED_ACTIVE_SUBSCRIPTION).unwrap();
        assert_eq!(
            pro_user.subscriptions.first().unwrap().id,
            mocked_subscription_obj.get("id").unwrap().as_str().unwrap()
        );

        assert_eq!(
            *app.permissions.calls.lock().await,
            [format!("new_user {user_id}"), format!("make_pro {user_id}")]
        );
    }

    #[tokio::test]
    async fn downgrade_in_case_subscription_due_payment() {
        let app = app().await;

        // POST user first so one exists in the database.
        let response = app.post_user("test-user", "basic").await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        let user: user::UserResponse = serde_json::from_slice(&body).unwrap();
        let user_id = &user.id;

        // Test upgrading to pro with a subscription id that points to a due session.
        let response = app
            .post_subscription(
                user_id,
                MOCKED_OVERDUE_PAYMENT_CHECKOUT_SUBSCRIPTION_ID,
                "pro",
                1,
            )
            .await;
        assert_eq!(response.status(), StatusCode::OK);

        // The auth service should call stripe to fetch the subscription with the sub id, and return a subscription
        // that is pending payment.
        let response = app
            .get_user_with_mocked_stripe(
                MOCKED_OVERDUE_PAYMENT_CHECKOUT_SUBSCRIPTION_ID,
                MOCKED_PAST_DUE_SUBSCRIPTION,
                user_id,
            )
            .await;

        assert_eq!(response.status(), StatusCode::OK);

        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        let actual_user: user::UserResponse = serde_json::from_slice(&body).unwrap();

        assert_eq!(actual_user.account_tier, AccountTier::PendingPaymentPro);

        assert_eq!(
            *app.permissions.calls.lock().await,
            [
                format!("new_user {user_id}"),
                format!("make_pro {user_id}"),
                format!("make_basic {user_id}"),
            ]
        );
    }

    #[tokio::test]
    async fn insert_and_increment_and_delete_rds_subscription() {
        let app = app().await;

        // Create user first so one exists in the database.
        let response = app.post_user("test-user", "basic").await;
        assert_eq!(response.status(), StatusCode::OK);

        // Extract the API key from the response so we can use it in a future request.
        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        let user: user::UserResponse = serde_json::from_slice(&body).unwrap();
        let user_id = &user.id;
        let basic_user_key = &user.key;

        // Make sure JWT does not allow any RDS instances
        let claim = app.get_claim(basic_user_key).await;
        assert!(claim.sub.starts_with("user_"));
        assert_eq!(claim.limits.rds_quota(), 0);

        // Send a request to insert an RDS subscription for the test user.
        let response = app
            .post_subscription(user_id, "sub_Eoarshy23pointInira", "rds", 1)
            .await;
        assert_eq!(response.status(), StatusCode::OK);

        // Fetch the user and verify they have an rds subscription.
        let response = app.get_user_typed(user_id).await;

        assert_eq!(
            response.subscriptions.len(),
            1,
            "there should be one subscription"
        );
        assert_eq!(
            response.subscriptions[0].r#type,
            user::SubscriptionType::Rds
        );
        assert_eq!(response.subscriptions[0].quantity, 1);

        // Make sure JWT has the quota
        let claim = app.get_claim(basic_user_key).await;
        assert_eq!(claim.limits.rds_quota(), 1);

        // Send another request to insert an RDS subscription for the user.
        // This uses a different subscription id to make sure we only keep record of one
        let response = app
            .post_subscription(user_id, "sub_IOhso230rakstr023soI", "rds", 4)
            .await;
        assert_eq!(response.status(), StatusCode::OK);

        // Make sure JWT has the new quota
        let claim = app.get_claim(basic_user_key).await;
        assert_eq!(claim.limits.rds_quota(), 4);

        // Send a request to delete an RDS subscription
        let response = app
            .delete_subscription(user_id, "sub_IOhso230rakstr023soI")
            .await;
        assert_eq!(response.status(), StatusCode::OK);

        // Make sure JWT is reset correctly
        let claim = app.get_claim(basic_user_key).await;
        assert_eq!(claim.limits.rds_quota(), 0);

        assert_eq!(
            *app.permissions.calls.lock().await,
            [format!("new_user {user_id}")]
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
        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        let user: user::UserResponse = serde_json::from_slice(&body).unwrap();
        let user_id = &user.id;

        // Upgrade user to pro
        let response = app
            .post_subscription(
                user_id,
                MOCKED_CANCELLEDPRO_CHECKOUT_SUBSCRIPTION_ID,
                "pro",
                1,
            )
            .await;
        assert_eq!(response.status(), StatusCode::OK);

        // Cancel subscription, this will be called by the console.
        let response = app
            .delete_subscription(user_id, MOCKED_CANCELLEDPRO_CHECKOUT_SUBSCRIPTION_ID)
            .await;
        assert_eq!(response.status(), StatusCode::OK);

        // Fetch the user to trigger a sync of the account tier to cancelled. The account should not
        // be downgraded to basic right away, since when we cancel subscriptions we pass in the
        // "cancel_at_period_end" end flag.
        let response = app
            .get_user_with_mocked_stripe(
                MOCKED_CANCELLEDPRO_CHECKOUT_SUBSCRIPTION_ID,
                MOCKED_CANCELLEDPRO_SUBSCRIPTION_ACTIVE,
                user_id,
            )
            .await;

        assert_eq!(response.status(), StatusCode::OK);

        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        let user: user::UserResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(user.account_tier, AccountTier::CancelledPro);

        // When called again at some later time, the subscription returned from stripe should be
        // cancelled.
        let response = app
            .get_user_with_mocked_stripe(
                MOCKED_CANCELLEDPRO_CHECKOUT_SUBSCRIPTION_ID,
                MOCKED_CANCELLEDPRO_SUBSCRIPTION_CANCELLED,
                user_id,
            )
            .await;
        assert_eq!(response.status(), StatusCode::OK);

        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        let user: user::UserResponse = serde_json::from_slice(&body).unwrap();

        assert_eq!(user.account_tier, AccountTier::Basic);

        assert_eq!(
            *app.permissions.calls.lock().await,
            [
                format!("new_user {user_id}"),
                format!("make_pro {user_id}"),
                format!("make_basic {user_id}"),
                format!("make_basic {user_id}"),
                format!("make_basic {user_id}"),
            ]
        );
    }
}
