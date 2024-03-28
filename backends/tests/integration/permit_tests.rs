mod needs_docker {
    use std::sync::OnceLock;

    use http::StatusCode;
    use permit_client_rs::apis::{
        resource_instances_api::{delete_resource_instance, list_resource_instances},
        users_api::list_users,
    };
    use serial_test::serial;
    use shuttle_backends::client::{
        permit::{Client, Error, ResponseContent},
        PermissionsDal,
    };
    use shuttle_common::claims::AccountTier;
    use shuttle_common_tests::permit_pdp::DockerInstance;
    use test_context::{test_context, AsyncTestContext};
    use uuid::Uuid;

    static PDP: OnceLock<DockerInstance> = OnceLock::new();

    #[ctor::dtor]
    fn cleanup() {
        println!("Cleaning up PDP container...");
        if let Some(p) = PDP.get() {
            p.cleanup()
        }
    }

    async fn clear_permit_state(client: &Client) {
        println!("Cleaning up Permit state ahead of test...");

        let users = list_users(
            &client.api,
            &client.proj_id,
            &client.env_id,
            None,
            None,
            None,
            None,
            Some(100),
        )
        .await
        .unwrap();

        for user in users.data {
            client.delete_user(&user.id.to_string()).await.unwrap();
        }

        let resources = list_resource_instances(
            &client.api,
            &client.proj_id,
            &client.env_id,
            None,
            None,
            None,
            None,
            Some(100),
            None,
        )
        .await
        .unwrap();

        for res in resources {
            delete_resource_instance(
                &client.api,
                &client.proj_id,
                &client.env_id,
                &res.id.to_string(),
            )
            .await
            .unwrap();
        }

        println!("Cleaning done.");
    }

    struct Wrap(Client);

    impl AsyncTestContext for Wrap {
        async fn setup() -> Self {
            let api_url = "https://api.eu-central-1.permit.io";
            let api_key = std::env::var("PERMIT_API_KEY")
                .expect("PERMIT_API_KEY to be set. You can copy the testing API key from the Testing environment on Permit.io.");

            PDP.get_or_init(|| {
                println!("Starting PDP container...");
                DockerInstance::new(&Uuid::new_v4().to_string(), api_url, &api_key)
            });

            let client = Client::new(
                api_url.to_owned(),
                PDP.get().unwrap().uri.clone(),
                // "http://localhost:19716".to_owned(),
                "default".to_owned(),
                std::env::var("PERMIT_ENV").unwrap_or_else(|_| "testing".to_owned()),
                api_key,
            );

            clear_permit_state(&client).await;

            Wrap(client)
        }

        async fn teardown(self) {}
    }

    #[test_context(Wrap)]
    #[tokio::test]
    #[serial]
    async fn test_user_flow(Wrap(client): &mut Wrap) {
        let u = "test_user";
        client.new_user(u).await.unwrap();
        let user = client.get_user(u).await.unwrap();

        // Can also get user by permit id
        let user_by_id = client.get_user(&user.id.to_string()).await.unwrap();

        assert_eq!(user.id, user_by_id.id);

        client.delete_user(u).await.unwrap();
        let res = client.get_user(u).await;

        assert!(matches!(
            res,
            Err(Error::ResponseError(ResponseContent {
                status: StatusCode::NOT_FOUND,
                ..
            }))
        ));
    }

    #[test_context(Wrap)]
    #[tokio::test]
    #[serial]
    async fn test_tiers_flow(Wrap(client): &mut Wrap) {
        let u = "tier_user";
        client.new_user(u).await.unwrap();
        let user = client.get_user(u).await.unwrap();

        assert_eq!(user.roles.as_ref().unwrap().len(), 1);
        assert_eq!(
            user.roles.as_ref().unwrap()[0].role,
            AccountTier::Basic.to_string()
        );

        client.make_pro(u).await.unwrap();
        let user = client.get_user(u).await.unwrap();

        assert_eq!(user.roles.as_ref().unwrap().len(), 1);
        assert_eq!(
            user.roles.as_ref().unwrap()[0].role,
            AccountTier::Pro.to_string()
        );

        client.make_basic(u).await.unwrap();
        let user = client.get_user(u).await.unwrap();

        assert_eq!(user.roles.as_ref().unwrap().len(), 1);
        assert_eq!(
            user.roles.as_ref().unwrap()[0].role,
            AccountTier::Basic.to_string()
        );
    }

    #[test_context(Wrap)]
    #[tokio::test]
    #[serial]
    async fn test_projects(Wrap(client): &mut Wrap) {
        let u1 = "user1";
        let u2 = "user2";
        client.new_user(u1).await.unwrap();
        client.new_user(u2).await.unwrap();

        const SLEEP: u64 = 500;

        tokio::time::sleep(tokio::time::Duration::from_millis(SLEEP)).await;
        let p1 = client.get_user_projects(u1).await.unwrap();

        assert!(p1.is_empty());

        client.create_project(u1, "proj1").await.unwrap();
        tokio::time::sleep(tokio::time::Duration::from_millis(SLEEP)).await;
        let p1 = client.get_user_projects(u1).await.unwrap();

        assert_eq!(p1.len(), 1);
        assert_eq!(p1[0].resource.as_ref().unwrap().key, "proj1");

        client.create_project(u1, "proj2").await.unwrap();
        tokio::time::sleep(tokio::time::Duration::from_millis(SLEEP)).await;
        let p1 = client.get_user_projects(u1).await.unwrap();

        assert_eq!(p1.len(), 2);

        client.delete_project("proj1").await.unwrap();
        tokio::time::sleep(tokio::time::Duration::from_millis(SLEEP)).await;
        let p1 = client.get_user_projects(u1).await.unwrap();

        assert_eq!(p1.len(), 1);
        assert_eq!(p1[0].resource.as_ref().unwrap().key, "proj2");

        let p2 = client.get_user_projects(u2).await.unwrap();

        assert!(p2.is_empty());
    }
}
