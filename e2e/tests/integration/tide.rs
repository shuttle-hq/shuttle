use crossterm::style::Color;

use crate::helpers;

#[test]
fn hello_world_tide() {
    let client = helpers::Services::new_docker("hello-world (tide)", Color::DarkYellow);
    client.deploy("tide/hello-world");

    let request_text = client
        .get("hello")
        .header("Host", "hello-world-tide-app.localhost.local")
        .send()
        .unwrap()
        .text()
        .unwrap();

    assert_eq!(request_text, "Hello, world!");
}
