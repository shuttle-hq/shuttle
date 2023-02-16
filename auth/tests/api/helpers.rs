use std::{
    net::{Ipv4Addr, SocketAddr},
    time::Duration,
};

pub struct TestApp {
    pub address: String,
    pub api_client: reqwest::Client,
}

/// Spawn a new application as a background task with a new database
/// for each test, ensuring test isolation.
pub async fn spawn_app() -> TestApp {
    let port = portpicker::pick_unused_port().unwrap();

    let address = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), port);

    _ = tokio::spawn(shuttle_auth::start(address, "sqlite::memory:"));

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
            .send()
            .await
            .expect("Failed to execute request.")
    }
}
