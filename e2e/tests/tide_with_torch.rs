use colored::Color;

mod helpers;

#[test]
fn hello_world() {
    let client = helpers::Services::new_docker("hello-world", Color::Cyan);
    client.deploy("../examples/tide/tide-with-torch");

    let request_text = client
        .get("torch")
        .header("Host", "tide-with-torch.shuttleapp.test")
        .send()
        .unwrap()
        .text()
        .unwrap();

    assert_eq!(request_text, "Hello with Rust Torch: [6, 2, 8, 2, 10]");
}
