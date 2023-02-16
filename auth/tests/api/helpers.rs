use std::{
    net::{Ipv4Addr, SocketAddr},
    time::Duration,
};

use shuttle_auth::InitArgs;

pub(crate) const ADMIN_KEY: &str = "my-api-key";

pub struct TestApp {
    pub address: String,
    pub api_client: reqwest::Client,
}

/// Spawn a new application as a background task with a new database
/// for each test, ensuring test isolation.
pub async fn spawn_app() -> TestApp {
    let port = portpicker::pick_unused_port().unwrap();

    let address = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), port);

    // Pass in init args to initialize the database with an admin user.
    let init_args = InitArgs {
        name: "admin".to_owned(),
        key: Some(ADMIN_KEY.to_owned()),
    };

    _ = tokio::spawn(shuttle_auth::start(
        "sqlite::memory:",
        address,
        Some(init_args),
    ));

    // Give the test-app time to start
    tokio::time::sleep(Duration::from_millis(500)).await;

    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();

    TestApp {
        address: format!("http://localhost:{port}"),
        api_client: client,
    }
}

impl TestApp {
    pub async fn post_user(&self, name: &str) -> reqwest::Response {
        self.api_client
            .post(&format!("{}/user/{}", &self.address, name))
            .bearer_auth(ADMIN_KEY)
            .send()
            .await
            .expect("Failed to execute request.")
    }
}
