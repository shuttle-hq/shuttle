use crossterm::style::Color;

use crate::helpers::{self, APPS_FQDN};

#[test]
fn hello_world_salvo() {
    let client =
        helpers::Services::new_docker("hello-world (salvo)", "salvo/hello-world", Color::DarkRed);
    client.deploy();

    let request_text = client
        .get("hello")
        .header("Host", format!("hello-world-salvo-app.{}", *APPS_FQDN))
        .send()
        .unwrap()
        .text()
        .unwrap();

    assert_eq!(request_text, "Hello, world!");
}
