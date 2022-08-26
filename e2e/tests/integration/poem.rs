use crossterm::style::Color;

use crate::helpers;

#[test]
fn hello_world_poem() {
    let client = helpers::Services::new_docker("hello-world (poem)", Color::Cyan);
    client.deploy("poem/hello-world");

    let request_text = client
        .get("hello")
        .header("Host", "hello-world-poem-app.localhost.local")
        .send()
        .unwrap()
        .text()
        .unwrap();

    assert_eq!(request_text, "Hello, world!");
}

#[test]
fn postgres_poem() {
    let client = helpers::Services::new_docker("postgres (poem)", Color::Blue);
    client.deploy("poem/postgres");

    let add_response = client
        .post("todo")
        .body("{\"note\": \"To the stars\"}")
        .header("Host", "postgres-poem-app.localhost.local")
        .header("content-type", "application/json")
        .send()
        .unwrap()
        .text()
        .unwrap();

    assert_eq!(add_response, "{\"id\":1,\"note\":\"To the stars\"}");

    let fetch_response: String = client
        .get("todo/1")
        .header("Host", "postgres-poem-app.localhost.local")
        .send()
        .unwrap()
        .text()
        .unwrap();

    assert_eq!(fetch_response, "{\"id\":1,\"note\":\"To the stars\"}");

    let secret_response: String = client
        .get("secret")
        .header("Host", "postgres-poem-app.localhost.local")
        .send()
        .unwrap()
        .text()
        .unwrap();

    assert_eq!(secret_response, "the contents of my API key");
}
