use colored::Color;

use crate::helpers;

#[test]
fn hello_world_poem() {
    let client = helpers::Services::new_docker("hello-world (poem)", Color::Cyan);
    client.deploy("poem/hello-world");

    let request_text = client
        .get("hello")
        .header("Host", "hello-world-poem-app.localhost.local")
        .send()
        .unwrap()
        .text()
        .unwrap();

    assert_eq!(request_text, "Hello, world!");
}
