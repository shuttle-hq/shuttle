use colored::Color;

mod helpers;

#[test]
fn hello_world() {
    let client = helpers::Services::new_docker("hello-world", Color::Cyan);
    client.deploy("../examples/tower/hello-world");

    let request_text = client
        .get("hello")
        .header("Host", "hello-world-tower-app.shuttleapp.test")
        .send()
        .unwrap()
        .text()
        .unwrap();

    assert_eq!(request_text, "Hello, world!");
}
