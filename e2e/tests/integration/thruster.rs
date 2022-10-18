use crossterm::style::Color;

use crate::helpers::{self, APPS_FQDN};

#[test]
fn hello_world_thruster() {
    let client = helpers::Services::new_docker("hello-world (thruster)", Color::DarkYellow);
    client.deploy("thruster/hello-world");

    let request_text = client
        .get("hello")
        .header("Host", format!("hello-world-thruster-app.{}", *APPS_FQDN))
        .send()
        .unwrap()
        .text()
        .unwrap();

    assert_eq!(request_text, "Hello, World!");
}
