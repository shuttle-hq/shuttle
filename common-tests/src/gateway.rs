use hyper::Method;
use serde::Serialize;
use wiremock::{
    matchers::{method, path},
    Mock, MockServer, ResponseTemplate,
};

pub async fn mocked_gateway_server() -> MockServer {
    let mock_server = MockServer::start().await;

    let projects = vec![Project {
        id: "01HMGS32BRKBFSY82WZE2WZZRY",
        name: "mock-project-1",
        state: "stopped",
        idle_minutes: 30,
    }];

    Mock::given(method(Method::GET))
        .and(path("/projects"))
        .respond_with(ResponseTemplate::new(200).set_body_json(projects))
        .mount(&mock_server)
        .await;

    mock_server
}

/// A denormalized project to make it easy to return mocked responses
#[derive(Serialize)]
struct Project<'a> {
    id: &'a str,
    name: &'a str,
    state: &'a str,
    idle_minutes: u64,
}
