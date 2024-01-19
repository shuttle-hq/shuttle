use hyper::Method;
use serde_json::json;
use wiremock::{
    matchers::{method, path},
    Mock, MockServer, ResponseTemplate,
};

pub async fn mocked_gateway_server() -> MockServer {
    let mock_server = MockServer::start().await;

    Mock::given(method(Method::GET))
        .and(path("/projects"))
        .respond_with(ResponseTemplate::new(200).set_body_json(vec![
            json!({"id": "01HMGS32BRKBFSY82WZE2WZZRY", "name": "mock-project-1", "state": "stopped", "idle_minutes": 30}),
        ]))
        .mount(&mock_server)
        .await;

    mock_server
}
