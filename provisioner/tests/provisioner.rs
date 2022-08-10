mod helpers;
use ctor::dtor;
use helpers::{exec_mongosh, exec_psql, DbType, DockerInstance};
use lazy_static::lazy_static;
use shuttle_proto::provisioner::shared;
use shuttle_provisioner::MyProvisioner;

lazy_static! {
    static ref PG: DockerInstance = DockerInstance::new(DbType::Postgres);
    static ref MONGODB: DockerInstance = DockerInstance::new(DbType::MongoDb);
}

#[dtor]
fn cleanup() {
    PG.cleanup();
    MONGODB.cleanup();
}

#[tokio::test]
async fn shared_db_role_does_not_exist() {
    let provisioner = MyProvisioner::new(
        &PG.uri,
        &MONGODB.uri,
        "fqdn".to_string(),
        "pg".to_string(),
        "mongodb".to_string(),
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
    )
    .await
    .unwrap();

    exec_psql("CREATE ROLE \"user-exist\" WITH LOGIN PASSWORD 'temp'");
    assert_eq!(
        exec_psql("SELECT passwd FROM pg_shadow WHERE usename = 'user-exist'",),
        "md5d44ae85dd21bda2a4f9946217adea2cc"
    );

    provisioner
        .request_shared_db("exist", shared::Engine::Postgres(String::new()))
        .await
        .unwrap();

    // Make sure password got cycled
    assert_ne!(
        exec_psql("SELECT passwd FROM pg_shadow WHERE usename = 'user-exist'",),
        "md5d44ae85dd21bda2a4f9946217adea2cc"
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

// TODO: more tests
#[tokio::test]
async fn request_shared_mongodb_with_private_role() {
    let provisioner = MyProvisioner::new(
        &PG.uri,
        &MONGODB.uri,
        "fqdn".to_string(),
        "pg".to_string(),
        "mongodb".to_string(),
    )
    .await
    .unwrap();

    assert!(!exec_mongosh("'show dbs'", None).contains("mongodb-provisioner-test"));

    provisioner
        .request_shared_db("provisioner-test", shared::Engine::Mongodb(String::new()))
        .await
        .unwrap();

    // `show dbs` command doesn't show DBs without collections, so instead we run the
    // `getUser` command in the new database
    let show_users = exec_mongosh(
        "db.getUser(\"user-provisioner-test\")",
        Some("mongodb-provisioner-test"),
    );

    assert!(show_users.contains("user-provisioner-test"));
    assert!(show_users.contains("role: 'readWrite', db: 'mongodb-provisioner-test'"));
}
