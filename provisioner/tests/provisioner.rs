mod helpers;
use ctor::dtor;
use helpers::{exec_mongosh, exec_psql, DbType, DockerInstance};
use once_cell::sync::Lazy;
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};
use serde_json::Value;
use shuttle_common::backends::subscription::{NewSubscriptionItem, SubscriptionItem};
use shuttle_proto::provisioner::shared;
use shuttle_provisioner::MyProvisioner;
use tonic::transport::Uri;
use wiremock::{
    matchers::{body_json, header, header_exists, method, path},
    MockServer, ResponseTemplate,
};

static PG: Lazy<DockerInstance> = Lazy::new(|| DockerInstance::new(DbType::Postgres));
static MONGODB: Lazy<DockerInstance> = Lazy::new(|| DockerInstance::new(DbType::MongoDb));

#[dtor]
fn cleanup() {
    PG.cleanup();
    MONGODB.cleanup();
}

#[tokio::test]
async fn correctly_calls_auth_service_to_add_rds_subscription_item() {
    let mock_server = MockServer::start().await;

    let provisioner = MyProvisioner::new(
        &PG.uri,
        &MONGODB.uri,
        "fqdn".to_string(),
        "pg".to_string(),
        "mongodb".to_string(),
        // Pass in the mock server's URI as the auth URI.
        mock_server.uri().parse::<Uri>().unwrap(),
    )
    .await
    .unwrap();

    let subscription_item =
        || NewSubscriptionItem::new("database-test-db", SubscriptionItem::AwsRds, 1);

    // Respond with a 200 for a correctly formed request.
    wiremock::Mock::given(method("POST"))
        .and(path("/users/subscription/items"))
        .and(header(CONTENT_TYPE, "application/json"))
        .and(header_exists(AUTHORIZATION))
        .and(body_json(subscription_item()))
        .respond_with(ResponseTemplate::new(200))
        .mount(&mock_server)
        .await;

    let res = provisioner
        .add_subscription_items("jwt", subscription_item())
        .await;

    assert!(res.is_ok());
}

mod needs_docker {
    use super::*;

    #[tokio::test]
    async fn shared_db_role_does_not_exist() {
        let provisioner = MyProvisioner::new(
            &PG.uri,
            &MONGODB.uri,
            "fqdn".to_string(),
            "pg".to_string(),
            "mongodb".to_string(),
            Uri::from_static("http://127.0.0.1:8008"),
        )
        .await
        .unwrap();

        assert_eq!(
            exec_psql("SELECT rolname FROM pg_roles WHERE rolname = 'user-not_exist'",),
            ""
        );

        provisioner
            .request_shared_db("not_exist", shared::Engine::Postgres(String::new()))
            .await
            .unwrap();

        assert_eq!(
            exec_psql("SELECT rolname FROM pg_roles WHERE rolname = 'user-not_exist'",),
            "user-not_exist"
        );
    }

    #[tokio::test]
    async fn shared_db_role_does_exist() {
        let provisioner = MyProvisioner::new(
            &PG.uri,
            &MONGODB.uri,
            "fqdn".to_string(),
            "pg".to_string(),
            "mongodb".to_string(),
            Uri::from_static("http://127.0.0.1:8008"),
        )
        .await
        .unwrap();

        exec_psql("CREATE ROLE \"user-exist\" WITH LOGIN PASSWORD 'temp'");
        let password = exec_psql("SELECT passwd FROM pg_shadow WHERE usename = 'user-exist'");

        provisioner
            .request_shared_db("exist", shared::Engine::Postgres(String::new()))
            .await
            .unwrap();

        // Make sure password got cycled
        assert_ne!(
            exec_psql("SELECT passwd FROM pg_shadow WHERE usename = 'user-exist'",),
            password
        );
    }

    #[tokio::test]
    #[should_panic(
        expected = "CreateRole(\"error returned from database: cannot insert multiple commands into a prepared statement\""
    )]
    async fn injection_safe() {
        let provisioner = MyProvisioner::new(
            &PG.uri,
            &MONGODB.uri,
            "fqdn".to_string(),
            "pg".to_string(),
            "mongodb".to_string(),
            Uri::from_static("http://127.0.0.1:8008"),
        )
        .await
        .unwrap();

        provisioner
            .request_shared_db(
                "new\"; CREATE ROLE \"injected",
                shared::Engine::Postgres(String::new()),
            )
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn shared_db_missing() {
        let provisioner = MyProvisioner::new(
            &PG.uri,
            &MONGODB.uri,
            "fqdn".to_string(),
            "pg".to_string(),
            "mongodb".to_string(),
            Uri::from_static("http://127.0.0.1:8008"),
        )
        .await
        .unwrap();

        assert_eq!(
            exec_psql("SELECT datname FROM pg_database WHERE datname = 'db-missing'",),
            ""
        );

        provisioner
            .request_shared_db("missing", shared::Engine::Postgres(String::new()))
            .await
            .unwrap();

        assert_eq!(
            exec_psql("SELECT datname FROM pg_database WHERE datname = 'db-missing'",),
            "db-missing"
        );
    }

    #[tokio::test]
    async fn shared_db_filled() {
        let provisioner = MyProvisioner::new(
            &PG.uri,
            &MONGODB.uri,
            "fqdn".to_string(),
            "pg".to_string(),
            "mongodb".to_string(),
            Uri::from_static("http://127.0.0.1:8008"),
        )
        .await
        .unwrap();

        exec_psql("CREATE ROLE \"user-filled\" WITH LOGIN PASSWORD 'temp'");
        exec_psql("CREATE DATABASE \"db-filled\" OWNER 'user-filled'");
        assert_eq!(
            exec_psql("SELECT datname FROM pg_database WHERE datname = 'db-filled'",),
            "db-filled"
        );

        provisioner
            .request_shared_db("filled", shared::Engine::Postgres(String::new()))
            .await
            .unwrap();

        assert_eq!(
            exec_psql("SELECT datname FROM pg_database WHERE datname = 'db-filled'",),
            "db-filled"
        );
    }

    #[tokio::test]
    async fn shared_mongodb_role_does_not_exist() {
        let provisioner = MyProvisioner::new(
            &PG.uri,
            &MONGODB.uri,
            "fqdn".to_string(),
            "pg".to_string(),
            "mongodb".to_string(),
            Uri::from_static("http://127.0.0.1:8008"),
        )
        .await
        .unwrap();

        let user = exec_mongosh("db.getUser(\"user-not_exist\")", Some("mongodb-not_exist"));
        assert_eq!(user, "null");

        provisioner
            .request_shared_db("not_exist", shared::Engine::Mongodb(String::new()))
            .await
            .unwrap();

        let user = exec_mongosh("db.getUser(\"user-not_exist\")", Some("mongodb-not_exist"));
        assert!(user.contains("mongodb-not_exist.user-not_exist"));
    }

    #[tokio::test]
    async fn shared_mongodb_role_does_exist() {
        let provisioner = MyProvisioner::new(
            &PG.uri,
            &MONGODB.uri,
            "fqdn".to_string(),
            "pg".to_string(),
            "mongodb".to_string(),
            Uri::from_static("http://127.0.0.1:8008"),
        )
        .await
        .unwrap();

        exec_mongosh(
            r#"db.createUser({ 
            user: "user-exist", 
            pwd: "secure_password", 
            roles: [
                { role: "readWrite", db: "mongodb-exist" }
            ]
        })"#,
            Some("mongodb-exist"),
        );

        let user: Value = serde_json::from_str(&exec_mongosh(
            r#"EJSON.stringify(db.getUser("user-exist", 
            { showCredentials: true }
        ))"#,
            Some("mongodb-exist"),
        ))
        .unwrap();

        // Extract the user's stored password hash key from the `getUser` output
        let user_stored_key = &user["credentials"]["SCRAM-SHA-256"]["storedKey"];
        assert_eq!(user["_id"], "mongodb-exist.user-exist");

        provisioner
            .request_shared_db("exist", shared::Engine::Mongodb(String::new()))
            .await
            .unwrap();

        let user: Value = serde_json::from_str(&exec_mongosh(
            r#"EJSON.stringify(db.getUser("user-exist", 
            { showCredentials: true }
        ))"#,
            Some("mongodb-exist"),
        ))
        .unwrap();

        // Make sure it's the same user
        assert_eq!(user["_id"], "mongodb-exist.user-exist");

        // Make sure password got cycled by comparing password hash keys
        let user_cycled_key = &user["credentials"]["SCRAM-SHA-256"]["storedKey"];
        assert_ne!(user_stored_key, user_cycled_key);
    }
}
