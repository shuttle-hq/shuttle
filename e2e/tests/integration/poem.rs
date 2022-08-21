use colored::Color;

use crate::helpers::{self, APPS_FQDN};

#[test]
fn hello_world_poem() {
    let client = helpers::Services::new_docker("hello-world (poem)", Color::Cyan);
    client.deploy("poem/hello-world");

    let request_text = client
        .get("hello")
        .header("Host", format!("hello-world-poem-app.{}", *APPS_FQDN))
        .send()
        .unwrap()
        .text()
        .unwrap();

    assert_eq!(request_text, "Hello, world!");
}

#[test]
fn postgres_poem() {
    let client = helpers::Services::new_docker("postgres", Color::Blue);
    client.deploy("poem/postgres");

    let add_response = client
        .post("todo")
        .body("{\"note\": \"To the stars\"}")
        .header("Host", format!("postgres-poem-app.{}", *APPS_FQDN))
        .header("content-type", "application/json")
        .send()
        .unwrap()
        .text()
        .unwrap();

    assert_eq!(add_response, "{\"id\":1,\"note\":\"To the stars\"}");

    let fetch_response: String = client
        .get("todo/1")
        .header("Host", format!("postgres-poem-app.{}", *APPS_FQDN))
        .send()
        .unwrap()
        .text()
        .unwrap();

    assert_eq!(fetch_response, "{\"id\":1,\"note\":\"To the stars\"}");

    let secret_response: String = client
        .get("secret")
        .header("Host", format!("postgres-poem-app.{}", *APPS_FQDN))
        .send()
        .unwrap()
        .text()
        .unwrap();

    assert_eq!(secret_response, "the contents of my API key");
}

#[test]
fn mongodb_poem() {
    let client = helpers::Services::new_docker("mongo (poem)", Color::Green);
    client.deploy("poem/mongodb");

    // post todo and get its generated objectId
    let add_response = client
        .post("todo")
        .body("{\"note\": \"To the stars\"}")
        .header("Host", format!("mongodb-poem-app.{}", *APPS_FQDN))
        .header("content-type", "application/json")
        .send()
        .unwrap()
        .text()
        .unwrap();

    // valid objectId is 24 char hex string
    assert_eq!(add_response.len(), 24, "response length mismatch: got: {}", add_response);

    let fetch_response: String = client
        .get(&format!("todo/{}", add_response))
        .header("Host", format!("mongodb-poem-app.{}", *APPS_FQDN))
        .send()
        .unwrap()
        .text()
        .unwrap();

    assert_eq!(fetch_response, "{\"note\":\"To the stars\"}");
}
