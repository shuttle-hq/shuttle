use crossterm::style::Color;

use crate::helpers;

#[test]
fn hello_world_tower() {
    let client = helpers::Services::new_docker("hello-world (tower)", Color::Yellow);
    client.deploy("tower/hello-world");

    let request_text = client
        .get("hello")
        .header("Host", "hello-world-tower-app.localhost.local")
        .send()
        .unwrap()
        .text()
        .unwrap();

    assert_eq!(request_text, "Hello, world!");
}
