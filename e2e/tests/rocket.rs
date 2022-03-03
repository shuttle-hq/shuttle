mod helpers;

#[test]
fn hello_world() {
    let _api = helpers::Api::new();
    helpers::deploy("../examples/rocket/hello-world");

    let request_text = reqwest::blocking::Client::new()
        .get("http://localhost:8000/hello")
        .header("Host", "hello-world-rocket-app.unveil.sh")
        .send()
        .unwrap()
        .text()
        .unwrap();

    assert_eq!(request_text, "Hello, world!");
}

#[test]
fn postgres() {
    let _api = helpers::Api::new();
    helpers::deploy("../examples/rocket/postgres");

    let client = reqwest::blocking::Client::new();
    let add_response = client
        .post("http://localhost:8000/todo")
        .body("{\"note\": \"To the stars\"}")
        .header("Host", "postgres-rocket-app.unveil.sh")
        .send()
        .unwrap()
        .text()
        .unwrap();

    assert_eq!(add_response, "{\"id\":1,\"note\":\"To the stars\"}");

    let fetch_response: String = client
        .get("http://localhost:8000/todo/1")
        .header("Host", "postgres-rocket-app.unveil.sh")
        .send()
        .unwrap()
        .text()
        .unwrap();

    assert_eq!(fetch_response, "{\"id\":1,\"note\":\"To the stars\"}");
}
