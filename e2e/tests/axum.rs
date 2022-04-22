use colored::Color;

mod helpers;

#[test]
fn hello_world() {
    let client = helpers::Api::new_docker("hello-world", Color::Cyan);
    client.deploy("../examples/axum/hello-world");

    let request_text = client
        .get("hello")
        .header("Host", "hello-world-axum-app.shuttleapp.test")
        .send()
        .unwrap()
        .text()
        .unwrap();

    assert_eq!(request_text, "Hello, world!");
}
