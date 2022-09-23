use colored::Color;

use crate::helpers::{self, APPS_FQDN};

#[test]
fn hello_world_salvo() {
    let client = helpers::Services::new_docker("hello-world (salvo)", Color::Cyan);
    client.deploy("salvo/hello-world");

    let request_text = client
        .get("hello")
        .header("Host", format!("hello-world-salvo-app.{}", *APPS_FQDN))
        .send()
        .unwrap()
        .text()
        .unwrap();

    assert_eq!(request_text, "Hello, world!");
}
