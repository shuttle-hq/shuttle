use colored::Color;

use crate::helpers;

#[test]
fn hello_world_rocket() {
    let client = helpers::Services::new_docker("hello-world (rocket)", Color::Green);
    client.deploy("rocket/hello-world");

    let request_text = client
        .get("hello")
        .header("Host", "hello-world-rocket-app.localhost.local")
        .send()
        .unwrap()
        .text()
        .unwrap();

    assert_eq!(request_text, "Hello, world!");
}

#[test]
fn postgres_rocket() {
    let client = helpers::Services::new_docker("postgres", Color::Blue);
    client.deploy("rocket/postgres");

    let add_response = client
        .post("todo")
        .body("{\"note\": \"To the stars\"}")
        .header("Host", "postgres-rocket-app.localhost.local")
        .send()
        .unwrap()
        .text()
        .unwrap();

    assert_eq!(add_response, "{\"id\":1,\"note\":\"To the stars\"}");

    let fetch_response: String = client
        .get("todo/1")
        .header("Host", "postgres-rocket-app.localhost.local")
        .send()
        .unwrap()
        .text()
        .unwrap();

    assert_eq!(fetch_response, "{\"id\":1,\"note\":\"To the stars\"}");

    let secret_response: String = client
        .get("secret")
        .header("Host", "postgres-rocket-app.localhost.local")
        .send()
        .unwrap()
        .text()
        .unwrap();

    assert_eq!(secret_response, "the contents of my API key");
}
