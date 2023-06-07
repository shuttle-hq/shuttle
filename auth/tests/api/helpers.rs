use std::{
    net::{Ipv4Addr, SocketAddr},
    time::Duration,
};

use portpicker::pick_unused_port;
use shuttle_auth::{EdDsaManager, Service, Sqlite};
use shuttle_proto::auth::{
    auth_client::AuthClient, auth_server::AuthServer, NewUser, UserRequest, UserResponse,
};
use tonic::{
    transport::{Channel, Server},
    Status,
};
use tonic::{Request, Response};

pub(crate) const ADMIN_KEY: &str = "ndh9z58jttoes3qv";

pub(crate) struct TestApp {
    pub client: AuthClient<Channel>,
}

/// Initialize a router with an in-memory sqlite database for each test.
pub(crate) async fn spawn_app() -> TestApp {
    let port = pick_unused_port().unwrap();
    let addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), port);

    let sqlite = Sqlite::new_in_memory().await;
    sqlite.insert_admin("admin", Some(ADMIN_KEY)).await;

    let mut server_builder =
        Server::builder().http2_keepalive_interval(Some(Duration::from_secs(60)));

    let key_manager = EdDsaManager::default();

    let svc = Service::new(sqlite, key_manager);
    let svc = AuthServer::new(svc);
    let router = server_builder.add_service(svc);

    let _ = tokio::spawn(router.serve(addr));

    let client = AuthClient::connect(format!("http://localhost:{port}"))
        .await
        .unwrap();

    TestApp { client }
}

impl TestApp {
    // pub async fn send_request(&self, request: Request<Body>) -> Response {
    //     self.router
    //         .clone()
    //         .oneshot(request)
    //         .await
    //         .expect("Failed to execute request.")
    // }

    // TODO: test that caller is admin
    pub async fn post_user(
        &mut self,
        name: &str,
        tier: &str,
    ) -> Result<Response<UserResponse>, Status> {
        Ok(self
            .client
            .post_user_request(Request::new(NewUser {
                account_name: name.to_string(),
                account_tier: tier.to_string(),
            }))
            .await?)
    }

    // TODO: test that caller is admin
    pub async fn get_user(&mut self, name: &str) -> Result<Response<UserResponse>, Status> {
        Ok(self
            .client
            .get_user_request(Request::new(UserRequest {
                account_name: name.to_string(),
            }))
            .await?)
    }
}
