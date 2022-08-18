use colored::Color;

use crate::helpers::{self, APPS_FQDN};

#[test]
fn hello_world_axum() {
    let client = helpers::Services::new_docker("hello-world (axum)", Color::Cyan);
    client.deploy("axum/hello-world");

    let request_text = client
        .get("hello")
        .header("Host", format!("hello-world-axum-app.{}", *APPS_FQDN))
        .send()
        .unwrap()
        .text()
        .unwrap();

    assert_eq!(request_text, "Hello, world!");
}
