use colored::Color;

use crate::helpers;

#[test]
fn hello_world_axum() {
    let client = helpers::Services::new_docker("hello-world (axum)", Color::Cyan);
    client.deploy("axum/hello-world");

    let request_text = client
        .get("hello")
        .header("Host", "hello-world-axum-app.localhost.local")
        .send()
        .unwrap()
        .text()
        .unwrap();

    assert_eq!(request_text, "Hello, world!");
}
