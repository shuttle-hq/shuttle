use crossterm::style::Color;

use crate::helpers::{self, APPS_FQDN};

#[test]
fn hello_world_tower() {
    let client = helpers::Services::new_docker("hello-world (tower)", Color::DarkYellow);
    client.deploy("tower/hello-world");

    let request_text = client
        .get("hello")
        .header("Host", format!("hello-world-tower-app.{}", *APPS_FQDN))
        .send()
        .unwrap()
        .text()
        .unwrap();

    assert_eq!(request_text, "Hello, world!");
}
