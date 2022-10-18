use crossterm::style::Color;

use crate::helpers::{self, APPS_FQDN};

#[test]
fn hello_world_tide() {
    let client = helpers::Services::new_docker("hello-world (tide)", Color::DarkYellow);
    client.deploy("tide/hello-world");

    let request_text = client
        .get("hello")
        .header("Host", format!("hello-world-tide-app.{}", *APPS_FQDN))
        .send()
        .unwrap()
        .text()
        .unwrap();

    assert_eq!(request_text, "Hello, world!");
}
